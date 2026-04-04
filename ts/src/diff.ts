import type { Value, Identity } from "./value.js";
import type { DefocusObject } from "./world.js";
import { World } from "./world.js";

/** Represents the difference between two world states. */
export interface WorldDiff {
  /** Objects that were added (full object data). */
  added: { [id: string]: DefocusObject };
  /** Objects that were removed. */
  removed: Identity[];
  /** Objects whose state changed (only changed keys).
   *  `null` value means the key was removed; non-null means it was set. */
  state_changes: { [id: string]: { [key: string]: Value | undefined } };
  /** Objects whose handlers changed.
   *  `null` value means the handler was removed; non-null means it was set. */
  handler_changes: { [id: string]: { [key: string]: Value | undefined } };
  /** Tick change (if any). */
  tick: number | null;
}

/** Create an empty diff. */
export function emptyDiff(): WorldDiff {
  return {
    added: {},
    removed: [],
    state_changes: {},
    handler_changes: {},
    tick: null,
  };
}

/** Returns true if this diff represents no changes. */
export function isDiffEmpty(diff: WorldDiff): boolean {
  return (
    Object.keys(diff.added).length === 0 &&
    diff.removed.length === 0 &&
    Object.keys(diff.state_changes).length === 0 &&
    Object.keys(diff.handler_changes).length === 0 &&
    diff.tick === null
  );
}

/** Deep equality check for Values. */
function valuesEqual(a: Value, b: Value): boolean {
  if (a === b) return true;
  if (a === null || b === null) return a === b;
  if (typeof a !== typeof b) return false;
  if (typeof a !== "object") return a === b;
  if (Array.isArray(a)) {
    if (!Array.isArray(b)) return false;
    if (a.length !== b.length) return false;
    return a.every((v, i) => valuesEqual(v, (b as Value[])[i]));
  }
  if (Array.isArray(b)) return false;
  const aRec = a as { [key: string]: Value };
  const bRec = b as { [key: string]: Value };
  const aKeys = Object.keys(aRec);
  const bKeys = Object.keys(bRec);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((k) => k in bRec && valuesEqual(aRec[k], bRec[k]));
}

/** Compute key-level diff between two record maps. */
function diffMaps(
  oldMap: { [key: string]: Value },
  newMap: { [key: string]: Value },
): { [key: string]: Value | undefined } {
  const changes: { [key: string]: Value | undefined } = {};

  for (const key of Object.keys(newMap)) {
    if (!(key in oldMap) || !valuesEqual(oldMap[key], newMap[key])) {
      changes[key] = newMap[key];
    }
  }

  for (const key of Object.keys(oldMap)) {
    if (!(key in newMap)) {
      changes[key] = undefined;
    }
  }

  return changes;
}

/** Compute the diff from `a` to `b`. "What changed to get from a to b?" */
export function diff(a: World, b: World): WorldDiff {
  const result = emptyDiff();

  if (a.tick !== b.tick) {
    result.tick = b.tick;
  }

  // Added objects
  for (const [id, obj] of b.objects) {
    if (!a.objects.has(id)) {
      result.added[id] = obj;
    }
  }

  // Removed objects
  for (const id of a.objects.keys()) {
    if (!b.objects.has(id)) {
      result.removed.push(id);
    }
  }

  // Changed objects
  for (const [id, oldObj] of a.objects) {
    const newObj = b.objects.get(id);
    if (!newObj) continue;

    const stateChanges = diffMaps(oldObj.state, newObj.state);
    if (Object.keys(stateChanges).length > 0) {
      result.state_changes[id] = stateChanges;
    }

    const handlerChanges = diffMaps(oldObj.handlers, newObj.handlers);
    if (Object.keys(handlerChanges).length > 0) {
      result.handler_changes[id] = handlerChanges;
    }
  }

  return result;
}

/** Apply a diff to a world, modifying it in-place. */
export function applyDiff(world: World, d: WorldDiff): void {
  if (d.tick !== null) {
    world.tick = d.tick;
  }

  for (const id of d.removed) {
    world.objects.delete(id);
  }

  for (const [id, obj] of Object.entries(d.added)) {
    world.objects.set(id, obj);
  }

  for (const [id, changes] of Object.entries(d.state_changes)) {
    const obj = world.objects.get(id);
    if (!obj) continue;
    for (const [key, value] of Object.entries(changes)) {
      if (value === undefined) {
        delete obj.state[key];
      } else {
        obj.state[key] = value;
      }
    }
  }

  for (const [id, changes] of Object.entries(d.handler_changes)) {
    const obj = world.objects.get(id);
    if (!obj) continue;
    for (const [key, value] of Object.entries(changes)) {
      if (value === undefined) {
        delete obj.handlers[key];
        obj.interface = obj.interface.filter((v) => v !== key);
      } else {
        obj.handlers[key] = value;
        if (!obj.interface.includes(key)) {
          obj.interface.push(key);
        }
      }
    }
  }
}
