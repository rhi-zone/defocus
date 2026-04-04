import type { Value, Expr, Identity } from "./value.js";
import { evalHandler } from "./eval.js";

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
}

export type Effect =
  | { type: "set"; key: string; value: Value }
  | { type: "send"; to: Identity; message: Message }
  | { type: "spawn"; object: DefocusObject }
  | { type: "remove"; id: Identity }
  | { type: "reply"; value: Value };

export function createObject(id: Identity): DefocusObject {
  return { id, state: {}, handlers: {}, interface: [], children: [] };
}

export function stub(id: Identity, verbs: string[]): DefocusObject {
  return { id, state: {}, handlers: {}, interface: verbs, children: [] };
}

export class World {
  objects = new Map<Identity, DefocusObject>();
  queue: Array<[Identity, Message, Identity | undefined]> = [];

  add(object: DefocusObject): void {
    this.objects.set(object.id, object);
  }

  send(to: Identity, message: Message): void {
    this.queue.push([to, message, undefined]);
  }

  /** Process the next queued message. Returns undefined if queue is empty,
   *  or an array of Reply values produced by this step. */
  step(): Value[] | undefined {
    const entry = this.queue.shift();
    if (!entry) return undefined;

    const [targetId, message, sender] = entry;
    const object = this.objects.get(targetId);
    if (!object) return [];

    const handler = object.handlers[message.verb];
    if (!handler) return [];

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
      objects[id] = {
        state: obj.state,
        handlers: obj.handlers,
        interface: obj.interface,
        children: obj.children,
      };
    }
    return { version: 1, objects };
  }

  static fromJSON(data: object): World {
    const root = data as { version?: number; objects?: { [id: string]: {
      state?: { [key: string]: Value };
      handlers?: { [verb: string]: Expr };
      interface?: string[];
      children?: Identity[];
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
      };
      world.add(obj);
    }
    return world;
  }
}
