import type { Value, Expr, Identity } from "./value.js";
import { evalHandler } from "./eval.js";
import type { Event, EventLog } from "./log.js";

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
  | { type: "send"; to: Identity; message: Message }
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
  queue: Array<[Identity, Message, Identity | undefined]> = [];
  log: EventLog | null = null;

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

    const effects = evalHandler(
      handler,
      message.payload,
      object.state,
      targetId,
      sender,
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
          this.queue.push([effect.to, effect.message, targetId]);
          break;
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
    return { version: 1, objects };
  }

  static fromJSON(data: object): World {
    const root = data as { version?: number; objects?: { [id: string]: {
      state?: { [key: string]: Value };
      handlers?: { [verb: string]: Expr };
      interface?: string[];
      children?: Identity[];
      prototype?: Identity | null;
    } } };

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
