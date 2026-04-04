/**
 * The Merchant's Shop — comprehensive integration test exercising all defocus features:
 * prototypes, attenuated refs, pattern matching, scheduling, user-defined functions,
 * event logging, branching/forking, and save/load via JSON serialization.
 */

import { describe, test, expect } from "bun:test";
import { World } from "./world.js";
import type { DefocusObject } from "./world.js";
import type { Value } from "./value.js";
import { forkAt } from "./log.js";

function sendMsg(world: World, to: string, verb: string, payload: Value = null): Value[] {
  world.send(to, { verb, payload });
  return world.drain(1000);
}

function buildWorld(): World {
  const world = new World();

  // -- item-prototype: shared handler for all items --
  const itemPrototype: DefocusObject = {
    id: "item-prototype",
    state: {},
    handlers: {
      look: ["perform", "reply", ["get-in", ["get", "state"], "description"]],
      take: [
        "do",
        ["perform", "reply",
          ["concat", "You pick up the ", ["get-in", ["get", "state"], "name"], "."]],
        ["perform", "set", "location", ["get", "sender"]],
      ],
    },
    interface: ["look", "take"],
    children: [],
    prototype: null,
  };
  world.add(itemPrototype);

  // -- local:shop — the room --
  const shop: DefocusObject = {
    id: "local:shop",
    state: {
      description: "A cluttered merchant's shop. Shelves line every wall.",
      merchant: { $ref: "local:merchant" },
      key: { $ref: "local:key", $verbs: ["look", "take"] },
      door: { $ref: "local:door", $verbs: ["look", "unlock"] },
    },
    handlers: {
      look: [
        "do",
        ["perform", "reply", ["get-in", ["get", "state"], "description"]],
        ["perform", "send", ["get-in", ["get", "state"], "merchant"], "look", null],
        ["perform", "send", ["get-in", ["get", "state"], "key"], "look", null],
        ["perform", "send", ["get-in", ["get", "state"], "door"], "look", null],
      ],
      enter: ["perform", "reply", "You step inside the shop."],
    },
    interface: ["look", "enter"],
    children: [],
    prototype: null,
  };
  world.add(shop);

  // -- local:merchant — NPC --
  const merchant: DefocusObject = {
    id: "local:merchant",
    state: {
      name: "Grizzled Merchant",
      mood: "suspicious",
      description: "A weathered merchant eyes you from behind the counter.",
    },
    handlers: {
      look: ["perform", "reply", ["get-in", ["get", "state"], "description"]],
      talk: [
        "match", ["get", "payload"],
        ["buy", ["do",
          ["perform", "reply", "What are you buying?"],
          ["perform", "set", "mood", "interested"],
        ]],
        ["haggle", ["if",
          ["=", ["get-in", ["get", "state"], "mood"], "interested"],
          ["do",
            ["perform", "set", "mood", "amused"],
            ["perform", "reply", "Ha! I like your spirit."],
          ],
          ["perform", "reply", "Buy something first."],
        ]],
        ["_", ["perform", "reply", "The merchant grunts."]],
      ],
    },
    interface: ["look", "talk"],
    children: [],
    prototype: null,
  };
  world.add(merchant);

  // -- local:key — inherits from item-prototype --
  const key: DefocusObject = {
    id: "local:key",
    state: {
      name: "Brass Key",
      description: "A tarnished brass key.",
    },
    handlers: {},
    interface: [],
    children: [],
    prototype: "item-prototype",
  };
  world.add(key);

  // -- local:door — stateful with scheduled effect and user-defined function --
  const door: DefocusObject = {
    id: "local:door",
    state: {
      locked: true,
      open: false,
      description: "A heavy oak door with an iron lock.",
    },
    handlers: {
      look: [
        // User-defined function: status-desc generates a combined status string
        "let", "status-desc",
        ["fn", ["locked", "open"], [
          "concat",
          ["if", ["get", "locked"], "Locked", "Unlocked"],
          " and ",
          ["if", ["get", "open"], "open", "closed"],
          ".",
        ]],
        ["perform", "reply",
          ["concat",
            ["get-in", ["get", "state"], "description"],
            " ",
            ["call", ["get", "status-desc"],
              ["get-in", ["get", "state"], "locked"],
              ["get-in", ["get", "state"], "open"],
            ],
          ],
        ],
      ],
      unlock: [
        "if", ["get-in", ["get", "state"], "locked"],
        ["do",
          ["perform", "set", "locked", false],
          ["perform", "reply", "Click. The lock turns."],
          ["perform", "schedule", 3, ["get", "self"], "creak", null],
        ],
        ["perform", "reply", "Already unlocked."],
      ],
      creak: ["perform", "reply", "The door creaks ominously..."],
      open: [
        "if", ["get-in", ["get", "state"], "locked"],
        ["perform", "reply", "The door is locked."],
        ["if", ["get-in", ["get", "state"], "open"],
          ["perform", "reply", "Already open."],
          ["do",
            ["perform", "set", "open", true],
            ["perform", "reply", "The door swings open."],
          ],
        ],
      ],
    },
    interface: ["look", "unlock", "creak", "open"],
    children: [],
    prototype: null,
  };
  world.add(door);

  return world;
}

describe("The Merchant's Shop", () => {
  test("full scenario", () => {
    const world = buildWorld();

    // 1. Enter the shop
    let replies = sendMsg(world, "local:shop", "enter");
    expect(replies).toEqual(["You step inside the shop."]);

    // 2. Look around — shop replies with description, then sends look to merchant/key/door
    replies = sendMsg(world, "local:shop", "look");
    expect(replies[0]).toBe("A cluttered merchant's shop. Shelves line every wall.");
    expect(replies[1]).toBe("A weathered merchant eyes you from behind the counter.");
    expect(replies[2]).toBe("A tarnished brass key.");
    expect(replies[3]).toBe("A heavy oak door with an iron lock. Locked and closed.");

    // 3. Examine the key directly (prototype handler fires with key's state)
    replies = sendMsg(world, "local:key", "look");
    expect(replies).toEqual(["A tarnished brass key."]);

    // 4. Try to open the door (locked)
    replies = sendMsg(world, "local:door", "open");
    expect(replies).toEqual(["The door is locked."]);

    // 5. Take the key (prototype handler)
    replies = sendMsg(world, "local:key", "take");
    expect(replies).toEqual(["You pick up the Brass Key."]);
    // Key's location set to sender (null for external sends)
    expect(world.objects.get("local:key")!.state.location).toBe(null);

    // 6. Talk to merchant "buy" — reply + mood change
    replies = sendMsg(world, "local:merchant", "talk", "buy");
    expect(replies).toEqual(["What are you buying?"]);
    expect(world.objects.get("local:merchant")!.state.mood).toBe("interested");

    // 7. Talk to merchant "haggle" — mood is now "interested"
    replies = sendMsg(world, "local:merchant", "talk", "haggle");
    expect(replies).toEqual(["Ha! I like your spirit."]);
    expect(world.objects.get("local:merchant")!.state.mood).toBe("amused");

    // 8. Unlock the door — reply + scheduled creak at tick 3
    replies = sendMsg(world, "local:door", "unlock");
    expect(replies).toEqual(["Click. The lock turns."]);
    expect(world.objects.get("local:door")!.state.locked).toBe(false);
    expect(world.schedule.has(3)).toBe(true);

    // 9. Advance to tick 3 — creak fires
    replies = world.advance(3);
    expect(replies).toEqual(["The door creaks ominously..."]);

    // 10. Open the door — now unlocked
    replies = sendMsg(world, "local:door", "open");
    expect(replies).toEqual(["The door swings open."]);
    expect(world.objects.get("local:door")!.state.open).toBe(true);

    // 11. Enable logging, do some actions
    world.enableLogging();
    replies = sendMsg(world, "local:merchant", "talk", "haggle");
    // Mood is "amused" now, not "interested", so haggle fails
    expect(replies).toEqual(["Buy something first."]);

    sendMsg(world, "local:merchant", "talk", "weather");

    const log = world.takeLog()!;
    expect(log).not.toBeNull();
    expect(log.events.length).toBe(2);
    expect(log.events[0].target).toBe("local:merchant");
    expect(log.events[0].message.verb).toBe("talk");
    expect(log.events[0].replies).toEqual(["Buy something first."]);
    expect(log.events[1].replies).toEqual(["The merchant grunts."]);

    // 12. Save world via JSON serialization
    const savedJson = JSON.stringify(world.toJSON());

    // 13. Branch the log — fork at the midpoint
    world.enableLogging();
    sendMsg(world, "local:merchant", "talk", "buy");
    sendMsg(world, "local:door", "look");
    const log2 = world.takeLog()!;
    expect(log2.events.length).toBe(2);

    // Load saved state as base for fork
    const baseWorld = World.fromJSON(JSON.parse(savedJson));

    // Fork at event index 1 (replay only the first event)
    const [forkedWorld, truncatedLog] = forkAt(baseWorld, log2, 1);
    expect(truncatedLog.events.length).toBe(1);

    // 14. In the branch, take a different action — verify divergence
    const forkedReplies = sendMsg(forkedWorld, "local:merchant", "talk", "haggle");
    // After "buy", mood is "interested", so haggle succeeds in the fork
    expect(forkedReplies).toEqual(["Ha! I like your spirit."]);
    expect(forkedWorld.objects.get("local:merchant")!.state.mood).toBe("amused");

    // Original world: merchant mood is "interested" (from the "buy" we did)
    expect(world.objects.get("local:merchant")!.state.mood).toBe("interested");

    // 15. Load the saved world — verify state matches pre-save
    const loaded = World.fromJSON(JSON.parse(savedJson));
    expect(loaded.objects.get("local:door")!.state.open).toBe(true);
    expect(loaded.objects.get("local:door")!.state.locked).toBe(false);
    expect(loaded.objects.get("local:merchant")!.state.mood).toBe("amused");
    expect(loaded.objects.get("local:key")!.prototype).toBe("item-prototype");
    expect(loaded.tick).toBe(3);
    expect(loaded.objects.size).toBe(5);

    // Verify loaded world is functional — door look still uses fn
    const loadedReplies = sendMsg(loaded, "local:door", "look");
    expect(loadedReplies).toEqual([
      "A heavy oak door with an iron lock. Unlocked and open.",
    ]);
  });

  test("attenuated refs block forbidden verbs", () => {
    const world = buildWorld();

    // Add a handler that sends "open" via the shop's attenuated door ref
    const shop = world.objects.get("local:shop")!;
    shop.handlers["try-open-door"] = [
      "perform", "send", ["get-in", ["get", "state"], "door"], "open", null,
    ];
    shop.interface.push("try-open-door");

    // First unlock the door so "open" would succeed if it got through
    sendMsg(world, "local:door", "unlock");

    // Try to open door through the attenuated ref (door ref only allows look + unlock)
    sendMsg(world, "local:shop", "try-open-door");

    // Door should still be closed — "open" verb was not in the allowed list
    expect(world.objects.get("local:door")!.state.open).toBe(false);

    // Verify "look" works through the attenuated ref
    shop.handlers["look-at-door"] = [
      "perform", "send", ["get-in", ["get", "state"], "door"], "look", null,
    ];
    shop.interface.push("look-at-door");

    const replies = sendMsg(world, "local:shop", "look-at-door");
    expect(replies.some((r) =>
      r === "A heavy oak door with an iron lock. Unlocked and closed."
    )).toBe(true);
  });
});
