import { describe, test, expect } from "bun:test";
import { MemoryBackend, Tee, Rolling, saveWorld, loadWorld } from "./persist.js";
import { World } from "./world.js";
import type { DefocusObject } from "./world.js";

describe("Persistence", () => {
  test("memory backend round-trip", () => {
    const backend = new MemoryBackend();
    const world = new World();

    const obj: DefocusObject = {
      id: "local:test",
      state: { x: 42, friend: { $ref: "local:other" } },
      handlers: {},
      interface: [],
      children: [],
    };
    world.add(obj);

    saveWorld(world, backend, "save-1");
    const loaded = loadWorld(backend, "save-1")!;

    expect(loaded).not.toBeNull();
    expect(loaded.objects.get("local:test")!.state.x).toBe(42);
    expect(loaded.objects.get("local:test")!.state.friend).toEqual({
      $ref: "local:other",
    });
  });

  test("tee writes to both", () => {
    const a = new MemoryBackend();
    const b = new MemoryBackend();
    const tee = new Tee(a, b);

    tee.save("key", "value");
    expect(a.load("key")).toBe("value");
    expect(b.load("key")).toBe("value");
  });

  test("rolling prunes oldest", () => {
    const rolling = new Rolling(new MemoryBackend(), 2, "save-");

    rolling.save("save-0", "first");
    rolling.save("save-1", "second");
    rolling.save("save-2", "third");

    expect(rolling.load("save-0")).toBeNull();
    expect(rolling.load("save-1")).toBe("second");
    expect(rolling.load("save-2")).toBe("third");
  });

  test("load missing key returns null", () => {
    const backend = new MemoryBackend();
    expect(loadWorld(backend, "nonexistent")).toBeNull();
  });
});
