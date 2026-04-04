import { describe, test, expect } from "bun:test";
import { World, createObject } from "./world.js";
import type { DefocusObject } from "./world.js";
import { diff, applyDiff, isDiffEmpty, emptyDiff } from "./diff.js";
import type { WorldDiff } from "./diff.js";

function makeObj(id: string, state: { [k: string]: any } = {}, handlers: { [k: string]: any } = {}): DefocusObject {
  return {
    id,
    state,
    handlers,
    interface: Object.keys(handlers),
    children: [],
    prototype: null,
  };
}

describe("WorldDiff", () => {
  test("no changes: diff of identical worlds is empty", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { x: 1 }));
    const w2 = w1.clone();

    const d = diff(w1, w2);
    expect(isDiffEmpty(d)).toBe(true);
  });

  test("state change: only changed property appears in diff", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { x: 1, y: 2 }));

    const w2 = w1.clone();
    w2.objects.get("obj:a")!.state.x = 42;

    const d = diff(w1, w2);
    expect(Object.keys(d.added).length).toBe(0);
    expect(d.removed.length).toBe(0);
    expect(Object.keys(d.handler_changes).length).toBe(0);
    expect(Object.keys(d.state_changes)).toEqual(["obj:a"]);
    expect(d.state_changes["obj:a"]).toEqual({ x: 42 });
  });

  test("object added: appears in added", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a"));

    const w2 = w1.clone();
    w2.add(makeObj("obj:b", { name: "new" }));

    const d = diff(w1, w2);
    expect(Object.keys(d.added)).toEqual(["obj:b"]);
    expect(d.removed.length).toBe(0);
  });

  test("object removed: appears in removed", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a"));
    w1.add(makeObj("obj:b"));

    const w2 = w1.clone();
    w2.objects.delete("obj:b");

    const d = diff(w1, w2);
    expect(Object.keys(d.added).length).toBe(0);
    expect(d.removed).toEqual(["obj:b"]);
  });

  test("handler change: appears in handler_changes", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", {}, { look: ["perform", "reply", "hello"] }));

    const w2 = w1.clone();
    const newHandler = ["perform", "reply", "goodbye"];
    w2.objects.get("obj:a")!.handlers.look = newHandler;

    const d = diff(w1, w2);
    expect(Object.keys(d.state_changes).length).toBe(0);
    expect(Object.keys(d.handler_changes)).toEqual(["obj:a"]);
    expect(d.handler_changes["obj:a"].look).toEqual(newHandler);
  });

  test("apply diff: roundtrip produces equivalent world", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { x: 1, y: 2 }));
    w1.add(makeObj("obj:b", { name: "bob" }));

    const w2 = new World();
    w2.tick = 5;
    w2.add(makeObj("obj:a", { x: 42, z: "new" }));
    w2.add(makeObj("obj:c", { name: "charlie" }));

    const d = diff(w1, w2);
    const result = w1.clone();
    applyDiff(result, d);

    expect(result.tick).toBe(w2.tick);
    expect(result.objects.size).toBe(w2.objects.size);

    for (const [id, expected] of w2.objects) {
      const actual = result.objects.get(id);
      expect(actual).toBeDefined();
      expect(actual!.state).toEqual(expected.state);
      expect(actual!.handlers).toEqual(expected.handlers);
    }
  });

  test("diff serialization: JSON roundtrip preserves diff", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { x: 1 }));

    const w2 = w1.clone();
    w2.objects.get("obj:a")!.state.x = 42;
    w2.add(makeObj("obj:b", { name: "new" }));

    const d = diff(w1, w2);
    const json = JSON.stringify(d);
    const restored: WorldDiff = JSON.parse(json);

    // Apply both and compare results
    const r1 = w1.clone();
    applyDiff(r1, d);
    const r2 = w1.clone();
    applyDiff(r2, restored);

    expect(r1.tick).toBe(r2.tick);
    expect(r1.objects.size).toBe(r2.objects.size);
    for (const [id, obj] of r1.objects) {
      expect(r2.objects.get(id)!.state).toEqual(obj.state);
    }
  });

  test("multiple changes: all captured in diff", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { hp: 100, mp: 50 }));
    w1.add(makeObj("obj:b", { alive: true }));
    w1.add(makeObj("obj:c", { temp: "delete me" }));

    const w2 = w1.clone();
    w2.objects.get("obj:a")!.state.hp = 80;
    w2.objects.get("obj:b")!.state.alive = false;
    w2.objects.delete("obj:c");
    w2.add(makeObj("obj:d", { new: true }));

    const d = diff(w1, w2);

    expect(Object.keys(d.state_changes).sort()).toEqual(["obj:a", "obj:b"]);
    expect(d.removed).toEqual(["obj:c"]);
    expect(Object.keys(d.added)).toEqual(["obj:d"]);

    // Roundtrip
    const result = w1.clone();
    applyDiff(result, d);
    expect(result.objects.size).toBe(w2.objects.size);
    for (const [id, expected] of w2.objects) {
      expect(result.objects.get(id)!.state).toEqual(expected.state);
    }
  });

  test("state key removal: None/undefined in diff", () => {
    const w1 = new World();
    w1.add(makeObj("obj:a", { keep: "yes", remove: "bye" }));

    const w2 = w1.clone();
    delete w2.objects.get("obj:a")!.state.remove;

    const d = diff(w1, w2);
    expect(d.state_changes["obj:a"].remove).toBeUndefined();
    expect("remove" in d.state_changes["obj:a"]).toBe(true);

    const result = w1.clone();
    applyDiff(result, d);
    expect(result.objects.get("obj:a")!.state.keep).toBe("yes");
    expect("remove" in result.objects.get("obj:a")!.state).toBe(false);
  });
});
