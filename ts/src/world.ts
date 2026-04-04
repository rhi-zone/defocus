import type { Value, Expr, Identity } from "./value.js";
import { evalHandler, evalHandlerWithLlm } from "./eval.js";
import type { Event, EventLog } from "./log.js";
import type { LlmProvider } from "./llm.js";

export interface Message {
  verb: string;
  payload: Value;
}

export interface DefocusObject {
  id: Identity;
  state: { [key: string]: Value };
  handlers: { [verb: string]: Expr };
  interface: string[];
  children: Identity[];
  prototype: Identity | null;
}

export type Effect =
  | { type: "set"; key: string; value: Value }
  | { type: "send"; to: Identity; allowedVerbs?: string[]; message: Message }
  | { type: "schedule"; at: number; to: Identity; message: Message }
  | { type: "spawn"; object: DefocusObject }
  | { type: "remove"; id: Identity }
  | { type: "reply"; value: Value };

export function createObject(id: Identity): DefocusObject {
  return { id, state: {}, handlers: {}, interface: [], children: [], prototype: null };
}

export function stub(id: Identity, verbs: string[]): DefocusObject {
  return { id, state: {}, handlers: {}, interface: verbs, children: [], prototype: null };
}

export class World {
  objects = new Map<Identity, DefocusObject>();
  tick = 0;
  schedule = new Map<number, Array<[Identity, Message]>>();
  queue: Array<[Identity, Message, Identity | undefined]> = [];
  log: EventLog | null = null;
  llm: LlmProvider | null = null;

  add(object: DefocusObject): void {
    this.objects.set(object.id, object);
  }

  send(to: Identity, message: Message): void {
    this.queue.push([to, message, undefined]);
  }

  /** Resolve a handler for a verb by walking the prototype chain. */
  private resolveHandler(startId: Identity, verb: string): Expr | undefined {
    const visited = new Set<Identity>();
    let currentId: Identity | null = startId;
    while (currentId !== null) {
      if (visited.has(currentId)) return undefined; // cycle
      visited.add(currentId);
      const obj = this.objects.get(currentId);
      if (!obj) return undefined;
      if (verb in obj.handlers) return obj.handlers[verb];
      currentId = obj.prototype;
    }
    return undefined;
  }

  /** Process the next queued message. Returns undefined if queue is empty,
   *  or an array of Reply values produced by this step. */
  step(): Value[] | undefined {
    const entry = this.queue.shift();
    if (!entry) return undefined;

    const [targetId, message, sender] = entry;
    const object = this.objects.get(targetId);
    if (!object) {
      if (this.log) {
        this.log.events.push({ target: targetId, message, sender, replies: [] });
      }
      return [];
    }

    // Walk prototype chain, but use original object's state
    const handler = this.resolveHandler(targetId, message.verb);
    if (handler === undefined) {
      if (this.log) {
        this.log.events.push({ target: targetId, message, sender, replies: [] });
      }
      return [];
    }

    const effects = evalHandlerWithLlm(
      handler,
      message.payload,
      object.state,
      targetId,
      sender,
      this.llm,
    );

    const replies: Value[] = [];
    for (const effect of effects) {
      switch (effect.type) {
        case "set": {
          const obj = this.objects.get(targetId);
          if (obj) obj.state[effect.key] = effect.value;
          break;
        }
        case "send":
          // Enforce capability attenuation: if the ref had a verb filter,
          // silently drop messages with disallowed verbs.
          if (effect.allowedVerbs && !effect.allowedVerbs.includes(effect.message.verb)) {
            break;
          }
          this.queue.push([effect.to, effect.message, targetId]);
          break;
        case "schedule": {
          const existing = this.schedule.get(effect.at);
          if (existing) {
            existing.push([effect.to, effect.message]);
          } else {
            this.schedule.set(effect.at, [[effect.to, effect.message]]);
          }
          break;
        }
        case "spawn":
          this.objects.set(effect.object.id, effect.object);
          break;
        case "remove":
          this.objects.delete(effect.id);
          break;
        case "reply":
          replies.push(effect.value);
          break;
      }
    }

    if (this.log) {
      this.log.events.push({ target: targetId, message, sender, replies: [...replies] });
    }

    return replies;
  }

  /** Process all queued messages. Returns all Reply values collected. */
  drain(limit: number): Value[] {
    const allReplies: Value[] = [];
    let count = 0;
    let replies: Value[] | undefined;
    while ((replies = this.step()) !== undefined) {
      allReplies.push(...replies);
      count++;
      if (count > limit) throw new Error(`drain exceeded ${limit} iterations`);
    }
    return allReplies;
  }

  /** Advance the logical clock to toTick, delivering all scheduled messages
   *  whose tick <= toTick. Processes them through the normal step loop.
   *  Returns all Reply values collected. */
  advance(toTick: number): Value[] {
    if (toTick < this.tick) {
      throw new Error(`cannot advance backward: current tick is ${this.tick}, requested ${toTick}`);
    }
    this.tick = toTick;

    // Collect and remove all entries with tick <= toTick
    const due: Array<[number, Array<[Identity, Message]>]> = [];
    for (const [tick, messages] of this.schedule) {
      if (tick <= toTick) {
        due.push([tick, messages]);
      }
    }
    // Sort by tick to deliver in order
    due.sort((a, b) => a[0] - b[0]);
    for (const [tick] of due) {
      this.schedule.delete(tick);
    }

    // Enqueue all due messages
    for (const [, messages] of due) {
      for (const [to, message] of messages) {
        this.queue.push([to, message, undefined]);
      }
    }

    return this.drain(10_000);
  }

  /** Advance the logical clock by one tick. Convenience wrapper. */
  advanceOne(): Value[] {
    return this.advance(this.tick + 1);
  }

  snapshot(): { [id: string]: DefocusObject } {
    return Object.fromEntries(this.objects);
  }

  toJSON(): object {
    const objects: { [id: string]: object } = {};
    for (const [id, obj] of this.objects) {
      const entry: { [key: string]: Value } = {
        state: obj.state,
        handlers: obj.handlers,
        interface: obj.interface,
        children: obj.children,
      };
      if (obj.prototype !== null) {
        entry.prototype = obj.prototype;
      }
      objects[id] = entry;
    }
    const result: { [key: string]: Value } = { version: 1, objects };
    if (this.tick > 0) {
      result.tick = this.tick;
    }
    if (this.schedule.size > 0) {
      const sched: { [tick: string]: Array<[Identity, Message]> } = {};
      for (const [tick, msgs] of this.schedule) {
        sched[String(tick)] = msgs;
      }
      result.schedule = sched;
    }
    return result;
  }

  static fromJSON(data: object): World {
    const root = data as {
      version?: number;
      tick?: number;
      schedule?: { [tick: string]: Array<[Identity, { verb: string; payload: Value }]> };
      objects?: { [id: string]: {
        state?: { [key: string]: Value };
        handlers?: { [verb: string]: Expr };
        interface?: string[];
        children?: Identity[];
        prototype?: Identity | null;
      } };
    };

    if (root.version !== 1) {
      throw new Error(`unsupported version: ${root.version}`);
    }
    if (!root.objects || typeof root.objects !== "object") {
      throw new Error("missing or invalid objects");
    }

    const world = new World();
    for (const [id, entry] of Object.entries(root.objects)) {
      const obj: DefocusObject = {
        id,
        state: entry.state ?? {},
        handlers: entry.handlers ?? {},
        interface: entry.interface ?? [],
        children: entry.children ?? [],
        prototype: entry.prototype ?? null,
      };
      world.add(obj);
    }

    if (typeof root.tick === "number") {
      world.tick = root.tick;
    }

    if (root.schedule && typeof root.schedule === "object") {
      for (const [tickStr, messages] of Object.entries(root.schedule)) {
        const tick = Number(tickStr);
        const msgs: Array<[Identity, Message]> = messages.map(
          ([to, msg]: [Identity, { verb: string; payload: Value }]) => [to, msg] as [Identity, Message],
        );
        world.schedule.set(tick, msgs);
      }
    }

    return world;
  }

  /** Enable event logging. Future step() calls will record events. */
  enableLogging(): void {
    if (!this.log) {
      this.log = { events: [] };
    }
  }

  /** Disable event logging. */
  disableLogging(): void {
    this.log = null;
  }

  /** Take the current log, leaving null in its place. */
  takeLog(): EventLog | null {
    const log = this.log;
    this.log = null;
    return log;
  }

  /** Deep clone the world (objects only, no queue or log). */
  clone(): World {
    return World.fromJSON(JSON.parse(JSON.stringify(this.toJSON())));
  }

  /** Re-dispatch all messages from a log in order, collecting all replies. */
  replay(log: EventLog): Value[] {
    const allReplies: Value[] = [];
    for (const event of log.events) {
      this.queue.push([event.target, event.message, event.sender]);
      let replies: Value[] | undefined;
      while ((replies = this.step()) !== undefined) {
        allReplies.push(...replies);
      }
    }
    return allReplies;
  }

  /** Given this world (pre-log) and a log, replay up to index,
   *  return the new world state and the truncated log. */
  forkAt(log: EventLog, index: number): [World, EventLog] {
    const truncated: EventLog = { events: log.events.slice(0, index) };
    const w = this.clone();
    w.replay(truncated);
    return [w, truncated];
  }
}
