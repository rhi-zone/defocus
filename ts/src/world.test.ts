import { describe, test, expect } from "bun:test";
import { World, createObject } from "./world.js";
import { evalHandler } from "./eval.js";
import type { DefocusObject, Effect } from "./world.js";
import type { Value, Expr } from "./value.js";
import { isRef, asRef, isTruthy } from "./value.js";

function sendMsg(world: World, to: string, verb: string, payload: Value = null): Value[] {
  world.send(to, { verb, payload });
  return world.drain(100);
}

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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
    };

    const frame: DefocusObject = {
      id: "local:frame",
      state: { doorOpen: false },
      handlers: {
        "door-opened": ["perform", "set", "doorOpen", true],
      },
      interface: ["door-opened"],
      children: [],
      prototype: null,
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
      prototype: null,
    };

    const frame: DefocusObject = {
      id: "local:frame",
      state: { doorOpen: false },
      handlers: {
        "door-opened": ["perform", "set", "doorOpen", true],
      },
      interface: ["door-opened"],
      children: [],
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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
      prototype: null,
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

  test("world serialization round-trip", () => {
    const world = new World();

    const room: DefocusObject = {
      id: "local:room",
      state: { description: "A dusty room.", door: { $ref: "local:door" } },
      handlers: {
        look: ["get-in", ["get", "state"], "description"],
      },
      interface: ["look"],
      children: ["local:door"],
    };

    const door: DefocusObject = {
      id: "local:door",
      state: { open: false },
      handlers: {
        open: ["perform", "set", "open", true],
      },
      interface: ["open"],
      children: [],
      prototype: null,
    };

    world.add(room);
    world.add(door);

    // Add to queue to verify it's not serialized
    world.send("local:door", { verb: "open", payload: null });

    const json = world.toJSON() as any;

    // Verify structure
    expect(json.version).toBe(1);
    expect(json.objects["local:room"].state.door).toEqual({ $ref: "local:door" });
    expect(json.objects["local:room"].children).toEqual(["local:door"]);
    // id should NOT be in the object body
    expect(json.objects["local:room"].id).toBeUndefined();

    // Round-trip
    const restored = World.fromJSON(json);

    // Queue should be empty
    expect(restored.queue.length).toBe(0);

    // Objects match
    expect(restored.objects.size).toBe(2);

    const restoredRoom = restored.objects.get("local:room")!;
    expect(restoredRoom.id).toBe("local:room");
    expect(restoredRoom.state.description).toBe("A dusty room.");
    expect(restoredRoom.state.door).toEqual({ $ref: "local:door" });
    expect(isRef(restoredRoom.state.door)).toBe(true);
    expect(asRef(restoredRoom.state.door)).toBe("local:door");
    expect(restoredRoom.interface).toEqual(["look"]);
    expect(restoredRoom.children).toEqual(["local:door"]);
    expect(restoredRoom.handlers.look).toBeDefined();

    const restoredDoor = restored.objects.get("local:door")!;
    expect(restoredDoor.state.open).toBe(false);
    expect(restoredDoor.interface).toEqual(["open"]);
    expect(restoredDoor.handlers.open).toBeDefined();
  });

  test("world serialization via JSON.stringify round-trip", () => {
    const world = new World();
    world.add({
      id: "local:obj",
      state: { ref: { $ref: "local:other" }, value: 42 },
      handlers: { ping: ["perform", "set", "ponged", true] },
      interface: ["ping"],
      children: [],
    });

    // Serialize through JSON string (simulates file save/load)
    const str = JSON.stringify(world.toJSON());
    const restored = World.fromJSON(JSON.parse(str));

    const obj = restored.objects.get("local:obj")!;
    expect(obj.state.ref).toEqual({ $ref: "local:other" });
    expect(isRef(obj.state.ref)).toBe(true);
    expect(obj.state.value).toBe(42);
    expect(restored.queue.length).toBe(0);
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
      prototype: null,
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

  test("interactive fiction scenario", () => {
    const world = new World();

    // Room
    const room: DefocusObject = {
      id: "local:room",
      state: {
        description: "A dimly lit stone chamber. A heavy wooden door stands to the north. An old woman sits in the corner.",
        door: { $ref: "local:door" },
        npc: { $ref: "local:npc" },
      },
      handlers: {
        look: [
          "do",
          ["perform", "reply", ["get-in", ["get", "state"], "description"]],
          ["perform", "reply",
            ["concat",
              "Door: ",
              ["if",
                ["get-in", ["get", "state"], "door"],
                "There is a door to the north.",
                "No door here.",
              ],
            ],
          ],
          ["perform", "reply",
            ["concat",
              "NPC: ",
              ["if",
                ["get-in", ["get", "state"], "npc"],
                "An old woman sits in the corner.",
                "Nobody is here.",
              ],
            ],
          ],
        ],
      },
      interface: ["look"],
      children: [],
      prototype: null,
    };

    // Door
    const door: DefocusObject = {
      id: "local:door",
      state: {
        open: false,
        description: "A heavy wooden door, reinforced with iron bands.",
      },
      handlers: {
        look: [
          "perform", "reply",
          ["concat",
            ["get-in", ["get", "state"], "description"],
            " It is ",
            ["if", ["get-in", ["get", "state"], "open"], "open", "closed"],
            ".",
          ],
        ],
        open: [
          "if",
          ["get-in", ["get", "state"], "open"],
          ["perform", "reply", "The door is already open."],
          ["do",
            ["perform", "set", "open", true],
            ["perform", "reply", "The door creaks open."],
          ],
        ],
        close: [
          "if",
          ["not", ["get-in", ["get", "state"], "open"]],
          ["perform", "reply", "The door is already closed."],
          ["do",
            ["perform", "set", "open", false],
            ["perform", "reply", "The door swings shut."],
          ],
        ],
      },
      interface: ["look", "open", "close"],
      children: [],
      prototype: null,
    };

    // NPC
    const npc: DefocusObject = {
      id: "local:npc",
      state: {
        name: "Old Woman",
        mood: "wary",
        description: "An old woman with sharp eyes watches you carefully.",
      },
      handlers: {
        look: ["perform", "reply", ["get-in", ["get", "state"], "description"]],
        talk: [
          "match", ["get", "payload"],
          ["greeting", ["do",
            ["perform", "set", "mood", "warm"],
            ["perform", "reply", "She nods slowly. 'Welcome, traveler.'"],
          ]],
          ["threat", ["do",
            ["perform", "set", "mood", "hostile"],
            ["perform", "reply", "Her eyes narrow. 'You'd best move along.'"],
          ]],
          ["_", ["perform", "reply", "She regards you silently."]],
        ],
      },
      interface: ["look", "talk"],
      children: [],
      prototype: null,
    };

    world.add(room);
    world.add(door);
    world.add(npc);

    // 1. Look at room
    let replies = sendMsg(world, "local:room", "look");
    expect(replies.length).toBe(3);
    expect(replies[0]).toBe(
      "A dimly lit stone chamber. A heavy wooden door stands to the north. An old woman sits in the corner.",
    );
    expect((replies[1] as string).includes("door")).toBe(true);
    expect((replies[2] as string).includes("NPC")).toBe(true);

    // 2. Look at door — verify mentions "closed"
    replies = sendMsg(world, "local:door", "look");
    expect(replies.length).toBe(1);
    expect((replies[0] as string).includes("closed")).toBe(true);

    // 3. Open door
    replies = sendMsg(world, "local:door", "open");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("The door creaks open.");
    expect(world.objects.get("local:door")!.state.open).toBe(true);

    // 4. Open door again
    replies = sendMsg(world, "local:door", "open");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("The door is already open.");

    // 5. Talk to NPC — greeting
    replies = sendMsg(world, "local:npc", "talk", "greeting");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("She nods slowly. 'Welcome, traveler.'");
    expect(world.objects.get("local:npc")!.state.mood).toBe("warm");

    // 6. Talk to NPC — threat
    replies = sendMsg(world, "local:npc", "talk", "threat");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("Her eyes narrow. 'You'd best move along.'");
    expect(world.objects.get("local:npc")!.state.mood).toBe("hostile");

    // 7. Talk to NPC — wildcard
    replies = sendMsg(world, "local:npc", "talk", "weather");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("She regards you silently.");
  });

  test("prototype: basic inheritance", () => {
    const world = new World();

    const proto: DefocusObject = {
      id: "proto:greeter",
      state: {},
      handlers: {
        greet: ["perform", "reply", ["get-in", ["get", "state"], "name"]],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };

    const instance: DefocusObject = {
      id: "local:instance",
      state: { name: "Alice" },
      handlers: {},
      interface: [],
      children: [],
      prototype: "proto:greeter",
    };

    world.add(proto);
    world.add(instance);

    const replies = sendMsg(world, "local:instance", "greet");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("Alice");
  });

  test("prototype: override", () => {
    const world = new World();

    const proto: DefocusObject = {
      id: "proto:greeter",
      state: {},
      handlers: {
        greet: ["perform", "reply", "proto hello"],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };

    const instance: DefocusObject = {
      id: "local:instance",
      state: {},
      handlers: {
        greet: ["perform", "reply", "instance hello"],
      },
      interface: ["greet"],
      children: [],
      prototype: "proto:greeter",
    };

    world.add(proto);
    world.add(instance);

    const replies = sendMsg(world, "local:instance", "greet");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("instance hello");
  });

  test("prototype: chain A -> B -> C", () => {
    const world = new World();

    const c: DefocusObject = {
      id: "proto:c",
      state: {},
      handlers: {
        greet: ["perform", "reply", "from C"],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };

    const b: DefocusObject = {
      id: "proto:b",
      state: {},
      handlers: {},
      interface: [],
      children: [],
      prototype: "proto:c",
    };

    const a: DefocusObject = {
      id: "local:a",
      state: {},
      handlers: {},
      interface: [],
      children: [],
      prototype: "proto:b",
    };

    world.add(c);
    world.add(b);
    world.add(a);

    const replies = sendMsg(world, "local:a", "greet");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("from C");
  });

  test("prototype: state isolation", () => {
    const world = new World();

    const proto: DefocusObject = {
      id: "proto:greeter",
      state: { name: "Proto" },
      handlers: {
        greet: ["perform", "reply", ["get-in", ["get", "state"], "name"]],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };

    const instance: DefocusObject = {
      id: "local:instance",
      state: { name: "Instance" },
      handlers: {},
      interface: [],
      children: [],
      prototype: "proto:greeter",
    };

    world.add(proto);
    world.add(instance);

    const replies = sendMsg(world, "local:instance", "greet");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("Instance");
  });

  test("prototype: stub with prototype", () => {
    const world = new World();

    const proto: DefocusObject = {
      id: "proto:greeter",
      state: {},
      handlers: {
        greet: ["perform", "reply", "hello from proto"],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };

    const instance: DefocusObject = {
      id: "local:instance",
      state: {},
      handlers: {},
      interface: ["greet"],
      children: [],
      prototype: "proto:greeter",
    };

    world.add(proto);
    world.add(instance);

    const replies = sendMsg(world, "local:instance", "greet");
    expect(replies.length).toBe(1);
    expect(replies[0]).toBe("hello from proto");
  });

  test("prototype: cycle protection", () => {
    const world = new World();

    const a: DefocusObject = {
      id: "local:a",
      state: {},
      handlers: {},
      interface: [],
      children: [],
      prototype: "local:b",
    };

    const b: DefocusObject = {
      id: "local:b",
      state: {},
      handlers: {},
      interface: [],
      children: [],
      prototype: "local:a",
    };

    world.add(a);
    world.add(b);

    // Should not infinite loop
    const replies = sendMsg(world, "local:a", "greet");
    expect(replies.length).toBe(0);
  });
});
