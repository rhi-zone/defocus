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
  | { type: "remove"; id: Identity };

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

  step(): boolean {
    const entry = this.queue.shift();
    if (!entry) return false;

    const [targetId, message, sender] = entry;
    const object = this.objects.get(targetId);
    if (!object) return true;

    const handler = object.handlers[message.verb];
    if (!handler) return true;

    const effects = evalHandler(
      handler,
      message.payload,
      object.state,
      targetId,
      sender,
    );

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
      }
    }

    return true;
  }

  drain(limit: number): number {
    let count = 0;
    while (this.step()) {
      count++;
      if (count > limit) throw new Error(`drain exceeded ${limit} iterations`);
    }
    return count;
  }

  snapshot(): { [id: string]: DefocusObject } {
    return Object.fromEntries(this.objects);
  }
}
