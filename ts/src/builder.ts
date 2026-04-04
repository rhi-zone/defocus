import type { Value, Expr, Identity } from "./value.js";
import type { DefocusObject } from "./world.js";
import { World } from "./world.js";

export class ObjectBuilder {
  private obj: DefocusObject;
  private parent: WorldBuilder;

  constructor(parent: WorldBuilder, id: Identity) {
    this.parent = parent;
    this.obj = {
      id,
      state: {},
      handlers: {},
      interface: [],
      children: [],
      prototype: null,
    };
  }

  state(key: string, value: Value): this {
    this.obj.state[key] = value;
    return this;
  }

  ref(key: string, targetId: Identity): this {
    this.obj.state[key] = { $ref: targetId };
    return this;
  }

  attenuatedRef(key: string, targetId: Identity, verbs: string[]): this {
    this.obj.state[key] = { $ref: targetId, $verbs: verbs };
    return this;
  }

  handler(verb: string, expr: Expr): this {
    if (!this.obj.interface.includes(verb)) {
      this.obj.interface.push(verb);
    }
    this.obj.handlers[verb] = expr;
    return this;
  }

  prototype(id: Identity): this {
    this.obj.prototype = id;
    return this;
  }

  child(id: Identity): this {
    this.obj.children.push(id);
    return this;
  }

  done(): WorldBuilder {
    this.parent._addObject(this.obj);
    return this.parent;
  }
}

export class WorldBuilder {
  private objects: DefocusObject[] = [];

  object(id: Identity): ObjectBuilder {
    return new ObjectBuilder(this, id);
  }

  /** @internal Used by ObjectBuilder.done() to register a completed object. */
  _addObject(obj: DefocusObject): void {
    this.objects.push(obj);
  }

  build(): World {
    const world = new World();
    for (const obj of this.objects) {
      world.add(obj);
    }
    return world;
  }
}
