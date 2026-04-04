import { describe, test, expect } from "bun:test";
import { World, createObject } from "./world.js";
import type { DefocusObject, Effect } from "./world.js";
import type { Value, Expr } from "./value.js";

describe("World", () => {
  test("door opens and notifies frame", () => {
    const world = new World();

    // A door that opens/closes and notifies a frame
    const door: DefocusObject = {
      id: "local:door",
      state: { open: false },
      handlers: {
        open: [
          "do",
          ["perform", "set", "open", true],
          ["perform", "send", "local:frame", "door-opened", null],
        ],
        close: ["perform", "set", "open", false],
      },
      interface: ["open", "close"],
      children: [],
    };

    // A frame that tracks whether its door is open
    const frame: DefocusObject = {
      id: "local:frame",
      state: { doorOpen: false },
      handlers: {
        "door-opened": ["perform", "set", "doorOpen", true],
      },
      interface: ["door-opened"],
      children: [],
    };

    world.add(door);
    world.add(frame);
    world.send("local:door", { verb: "open", payload: null });
    world.drain(100);

    expect(world.objects.get("local:door")!.state.open).toBe(true);
    expect(world.objects.get("local:frame")!.state.doorOpen).toBe(true);
  });

  test("conditional handler based on state", () => {
    const world = new World();

    const light: DefocusObject = {
      id: "local:light",
      state: { on: false },
      handlers: {
        toggle: [
          "if",
          ["get-in", ["get", "state"], "on"],
          ["perform", "set", "on", false],
          ["perform", "set", "on", true],
        ],
      },
      interface: ["toggle"],
      children: [],
    };

    world.add(light);

    world.send("local:light", { verb: "toggle", payload: null });
    world.drain(100);
    expect(world.objects.get("local:light")!.state.on).toBe(true);

    world.send("local:light", { verb: "toggle", payload: null });
    world.drain(100);
    expect(world.objects.get("local:light")!.state.on).toBe(false);
  });

  test("pattern matching on payload", () => {
    const world = new World();

    const npc: DefocusObject = {
      id: "local:npc",
      state: { mood: "neutral" },
      handlers: {
        greet: [
          "match",
          ["get", "payload"],
          ["friendly", ["perform", "set", "mood", "happy"]],
          ["hostile", ["perform", "set", "mood", "angry"]],
          ["_", ["perform", "set", "mood", "confused"]],
        ],
      },
      interface: ["greet"],
      children: [],
    };

    world.add(npc);

    world.send("local:npc", { verb: "greet", payload: "friendly" });
    world.drain(100);
    expect(world.objects.get("local:npc")!.state.mood).toBe("happy");

    world.send("local:npc", { verb: "greet", payload: "unknown" });
    world.drain(100);
    expect(world.objects.get("local:npc")!.state.mood).toBe("confused");
  });

  test("stub objects satisfy interface without handlers", () => {
    const world = new World();

    const server: DefocusObject = {
      id: "local:server",
      state: {},
      handlers: {},
      interface: ["ping", "query"],
      children: [],
    };

    world.add(server);

    // Message is dispatched but nothing happens — stub doesn't crash
    world.send("local:server", { verb: "ping", payload: null });
    world.drain(100);

    expect(world.objects.get("local:server")).toBeDefined();
    expect(world.objects.get("local:server")!.interface).toEqual([
      "ping",
      "query",
    ]);
  });

  test("object spawns another object", () => {
    const world = new World();

    // A factory that spawns items when asked
    const factory: DefocusObject = {
      id: "local:factory",
      state: { count: 0 },
      handlers: {
        produce: [
          "do",
          [
            "perform",
            "set",
            "count",
            ["+", ["get-in", ["get", "state"], "count"], 1],
          ],
        ],
      },
      interface: ["produce"],
      children: [],
    };

    world.add(factory);
    world.send("local:factory", { verb: "produce", payload: null });
    world.send("local:factory", { verb: "produce", payload: null });
    world.drain(100);

    expect(world.objects.get("local:factory")!.state.count).toBe(2);
  });
});
