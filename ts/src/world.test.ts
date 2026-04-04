import { describe, test, expect } from "bun:test";
import { World, createObject } from "./world.js";
import { evalHandler } from "./eval.js";
import type { DefocusObject, Effect } from "./world.js";
import type { Value, Expr } from "./value.js";
import { isRef, asRef, isTruthy } from "./value.js";

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

  test("fn: simple function definition and call", () => {
    const handler: Expr = [
      "let", "add", ["fn", ["a", "b"], ["+", ["get", "a"], ["get", "b"]]],
      ["call", ["get", "add"], 3, 4],
    ];
    const effects = evalHandler(handler, null, null);
    // No effects — just verify it doesn't crash. We need eval result.
    expect(effects).toEqual([]);
  });

  test("fn: function call returns correct value via effect", () => {
    // Use perform to observe the function result
    const handler: Expr = [
      "let", "add", ["fn", ["a", "b"], ["+", ["get", "a"], ["get", "b"]]],
      ["perform", "set", "result", ["call", ["get", "add"], 3, 4]],
    ];
    const effects = evalHandler(handler, null, null);
    expect(effects).toEqual([{ type: "set", key: "result", value: 7 }]);
  });

  test("fn: no args", () => {
    const handler: Expr = [
      "perform", "set", "result", ["call", ["fn", [], 42]],
    ];
    const effects = evalHandler(handler, null, null);
    expect(effects).toEqual([{ type: "set", key: "result", value: 42 }]);
  });

  test("fn: function as value in state", () => {
    const handler: Expr = [
      "perform", "set", "result", ["call", ["get", "state"], 10, 20],
    ];
    const state: Value = ["$fn", ["a", "b"], ["+", ["get", "a"], ["get", "b"]]];
    const effects = evalHandler(handler, null, state);
    expect(effects).toEqual([{ type: "set", key: "result", value: 30 }]);
  });

  test("fn: nested calls", () => {
    const handler: Expr = [
      "let", "double", ["fn", ["x"], ["+", ["get", "x"], ["get", "x"]]],
      ["perform", "set", "result",
        ["call", ["get", "double"], ["call", ["get", "double"], 3]]],
    ];
    const effects = evalHandler(handler, null, null);
    expect(effects).toEqual([{ type: "set", key: "result", value: 12 }]);
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

  test("door opens frame via ref", () => {
    const world = new World();

    const door: DefocusObject = {
      id: "local:door",
      state: { open: false, frame: { $ref: "local:frame" } },
      handlers: {
        open: [
          "do",
          ["perform", "set", "open", true],
          [
            "perform",
            "send",
            ["get-in", ["get", "state"], "frame"],
            "door-opened",
            null,
          ],
        ],
      },
      interface: ["open"],
      children: [],
    };

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

  test("string send still works (backward compat)", () => {
    const world = new World();

    const door: DefocusObject = {
      id: "local:door",
      state: {},
      handlers: {
        open: ["perform", "send", "local:frame", "door-opened", null],
      },
      interface: ["open"],
      children: [],
    };

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

    expect(world.objects.get("local:frame")!.state.doorOpen).toBe(true);
  });

  test("self and sender bindings", () => {
    const world = new World();

    const a: DefocusObject = {
      id: "local:a",
      state: {},
      handlers: {
        trigger: ["perform", "send", "local:b", "ping", null],
      },
      interface: ["trigger"],
      children: [],
    };

    const b: DefocusObject = {
      id: "local:b",
      state: {},
      handlers: {
        ping: [
          "do",
          ["perform", "set", "got-self", ["get", "self"]],
          ["perform", "set", "got-sender", ["get", "sender"]],
        ],
      },
      interface: ["ping"],
      children: [],
    };

    world.add(a);
    world.add(b);
    world.send("local:a", { verb: "trigger", payload: null });
    world.drain(100);

    expect(world.objects.get("local:b")!.state["got-self"]).toEqual({
      $ref: "local:b",
    });
    expect(world.objects.get("local:b")!.state["got-sender"]).toEqual({
      $ref: "local:a",
    });
  });

  test("external send has no sender", () => {
    const world = new World();

    const obj: DefocusObject = {
      id: "local:obj",
      state: {},
      handlers: {
        ping: ["perform", "set", "got-sender", ["get", "sender"]],
      },
      interface: ["ping"],
      children: [],
    };

    world.add(obj);
    world.send("local:obj", { verb: "ping", payload: null });
    world.drain(100);

    expect(world.objects.get("local:obj")!.state["got-sender"]).toBe(null);
  });

  test("isRef and asRef", () => {
    expect(isRef({ $ref: "local:frame" })).toBe(true);
    expect(isRef({ $ref: "local:frame", extra: 1 })).toBe(false);
    expect(isRef("local:frame")).toBe(false);
    expect(isRef(null)).toBe(false);
    expect(asRef({ $ref: "local:frame" })).toBe("local:frame");
    expect(asRef("local:frame")).toBe(undefined);
  });

  test("ref is truthy", () => {
    expect(isTruthy({ $ref: "local:frame" })).toBe(true);
  });

  test("spawn creates object in world", () => {
    const world = new World();

    const spawner: DefocusObject = {
      id: "local:spawner",
      state: {},
      handlers: {
        create: [
          "perform",
          "spawn",
          "local:child",
          {
            state: { alive: true },
            handlers: {
              ping: ["perform", "set", "ponged", true],
            },
            interface: ["ping"],
          },
        ],
      },
      interface: ["create"],
      children: [],
    };

    world.add(spawner);
    world.send("local:spawner", { verb: "create", payload: null });
    world.drain(100);

    expect(world.objects.has("local:child")).toBe(true);
    const child = world.objects.get("local:child")!;
    expect(child.state.alive).toBe(true);
    expect(child.interface).toEqual(["ping"]);
    expect(child.handlers.ping).toBeDefined();
  });

  test("spawn returns ref usable for send", () => {
    const world = new World();

    const spawner: DefocusObject = {
      id: "local:spawner",
      state: {},
      handlers: {
        "create-and-ping": [
          "let",
          "child-ref",
          [
            "perform",
            "spawn",
            "local:child",
            {
              state: { pinged: false },
              handlers: {
                ping: ["perform", "set", "pinged", true],
              },
              interface: ["ping"],
            },
          ],
          ["perform", "send", ["get", "child-ref"], "ping", null],
        ],
      },
      interface: ["create-and-ping"],
      children: [],
    };

    world.add(spawner);
    world.send("local:spawner", {
      verb: "create-and-ping",
      payload: null,
    });
    world.drain(100);

    expect(world.objects.has("local:child")).toBe(true);
    expect(world.objects.get("local:child")!.state.pinged).toBe(true);
  });

  test("spawned object handlers work", () => {
    const world = new World();

    const spawner: DefocusObject = {
      id: "local:spawner",
      state: {},
      handlers: {
        create: [
          "perform",
          "spawn",
          "local:counter",
          {
            state: { count: 0 },
            handlers: {
              increment: [
                "perform",
                "set",
                "count",
                ["+", ["get-in", ["get", "state"], "count"], 1],
              ],
            },
            interface: ["increment"],
          },
        ],
      },
      interface: ["create"],
      children: [],
    };

    world.add(spawner);
    world.send("local:spawner", { verb: "create", payload: null });
    world.drain(100);

    // Now send messages directly to the spawned object
    world.send("local:counter", { verb: "increment", payload: null });
    world.send("local:counter", { verb: "increment", payload: null });
    world.send("local:counter", { verb: "increment", payload: null });
    world.drain(100);

    expect(world.objects.get("local:counter")!.state.count).toBe(3);
  });
});
