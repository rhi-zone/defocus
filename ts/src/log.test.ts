import { describe, test, expect } from "bun:test";
import { World } from "./world.js";
import type { DefocusObject } from "./world.js";
import type { Value } from "./value.js";
import type { EventLog } from "./log.js";
import { branchAt, replayFrom, forkAt } from "./log.js";

function sendMsg(world: World, to: string, verb: string, payload: Value = null): Value[] {
  world.send(to, { verb, payload });
  return world.drain(100);
}

function makeCounterWorld(): World {
  const world = new World();
  const counter: DefocusObject = {
    id: "local:counter",
    state: { count: 0 },
    handlers: {
      increment: [
        "do",
        ["perform", "set", "count", ["+", ["get-in", ["get", "state"], "count"], 1]],
        ["perform", "reply", ["+", ["get-in", ["get", "state"], "count"], 1]],
      ],
    },
    interface: ["increment"],
    children: [],
    prototype: null,
  };
  world.add(counter);
  return world;
}

describe("EventLog", () => {
  test("log captures events", () => {
    const world = makeCounterWorld();
    world.enableLogging();

    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");

    const log = world.takeLog()!;
    expect(log).not.toBeNull();
    expect(log.events.length).toBe(3);

    expect(log.events[0].target).toBe("local:counter");
    expect(log.events[0].message.verb).toBe("increment");
    expect(log.events[0].sender).toBeUndefined();
    expect(log.events[0].replies).toEqual([1]);

    expect(log.events[1].replies).toEqual([2]);
    expect(log.events[2].replies).toEqual([3]);

    // Log was taken — should be null now
    expect(world.takeLog()).toBeNull();
  });

  test("replay produces same state", () => {
    const world = makeCounterWorld();
    const snapshot = world.clone();
    world.enableLogging();

    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");

    const log = world.takeLog()!;

    // Replay on a fresh copy
    const [replayed] = replayFrom(snapshot, log);

    expect(replayed.objects.get("local:counter")!.state.count)
      .toBe(world.objects.get("local:counter")!.state.count);
    expect(replayed.objects.get("local:counter")!.state.count).toBe(3);
  });

  test("branch at point", () => {
    const world = makeCounterWorld();
    const snapshot = world.clone();
    world.enableLogging();

    for (let i = 0; i < 5; i++) {
      sendMsg(world, "local:counter", "increment");
    }
    const log = world.takeLog()!;
    expect(log.events.length).toBe(5);

    // Branch at message 3
    const [branched, truncatedLog] = forkAt(snapshot, log, 3);

    expect(truncatedLog.events.length).toBe(3);
    expect(branched.objects.get("local:counter")!.state.count).toBe(3);
  });

  test("branch and diverge", () => {
    const world = makeCounterWorld();

    // Add a second object
    const npc: DefocusObject = {
      id: "local:npc",
      state: { mood: "neutral" },
      handlers: {
        greet: [
          "do",
          ["perform", "set", "mood", ["get", "payload"]],
          ["perform", "reply", ["get", "payload"]],
        ],
      },
      interface: ["greet"],
      children: [],
      prototype: null,
    };
    world.add(npc);

    const snapshot = world.clone();
    world.enableLogging();

    // Send 3 increment messages
    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");

    const log = world.takeLog()!;

    // Branch at message 3 and send a different 4th message
    const [branched] = forkAt(snapshot, log, 3);
    expect(branched.objects.get("local:counter")!.state.count).toBe(3);

    // Diverge: send a greet to npc instead of another increment
    sendMsg(branched, "local:npc", "greet", "happy");

    // Continue original: send a 4th increment
    sendMsg(world, "local:counter", "increment");

    // Verify divergence
    expect(world.objects.get("local:counter")!.state.count).toBe(4);
    expect(branched.objects.get("local:counter")!.state.count).toBe(3);
    expect(branched.objects.get("local:npc")!.state.mood).toBe("happy");
    expect(world.objects.get("local:npc")!.state.mood).toBe("neutral");
  });

  test("event log serialization roundtrip", () => {
    const world = makeCounterWorld();
    world.enableLogging();

    sendMsg(world, "local:counter", "increment");
    sendMsg(world, "local:counter", "increment");

    const log = world.takeLog()!;
    const json = JSON.stringify(log);
    const restored: EventLog = JSON.parse(json);

    expect(restored.events.length).toBe(2);
    expect(restored.events[0].target).toBe("local:counter");
    expect(restored.events[0].message.verb).toBe("increment");
    expect(restored.events[0].replies).toEqual([1]);
    expect(restored.events[1].replies).toEqual([2]);
  });
});
