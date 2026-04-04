import { describe, test, expect } from "bun:test";
import { WorldBuilder } from "./builder.js";
import type { Value } from "./value.js";

describe("WorldBuilder", () => {
  test("builds objects with correct state", () => {
    const world = new WorldBuilder()
      .object("local:room")
        .state("description", "A dusty room.")
        .state("lit", true)
        .done()
      .build();

    const room = world.objects.get("local:room");
    expect(room).toBeDefined();
    expect(room!.id).toBe("local:room");
    expect(room!.state.description).toBe("A dusty room.");
    expect(room!.state.lit).toBe(true);
  });

  test("builds objects with refs", () => {
    const world = new WorldBuilder()
      .object("local:room")
        .ref("door", "local:door")
        .done()
      .object("local:door")
        .state("open", false)
        .done()
      .build();

    const room = world.objects.get("local:room");
    expect(room!.state.door).toEqual({ $ref: "local:door" });
  });

  test("builds objects with attenuated refs", () => {
    const world = new WorldBuilder()
      .object("local:room")
        .attenuatedRef("door", "local:door", ["look", "open"])
        .done()
      .build();

    const room = world.objects.get("local:room");
    expect(room!.state.door).toEqual({ $ref: "local:door", $verbs: ["look", "open"] });
  });

  test("builds objects with handlers and auto-populates interface", () => {
    const world = new WorldBuilder()
      .object("local:door")
        .handler("open", ["perform", "set", "open", true])
        .handler("close", ["perform", "set", "open", false])
        .done()
      .build();

    const door = world.objects.get("local:door");
    expect(door!.handlers.open).toEqual(["perform", "set", "open", true]);
    expect(door!.handlers.close).toEqual(["perform", "set", "open", false]);
    expect(door!.interface).toEqual(["open", "close"]);
  });

  test("does not duplicate verbs in interface", () => {
    const world = new WorldBuilder()
      .object("local:obj")
        .handler("ping", ["perform", "reply", "pong"])
        .handler("ping", ["perform", "reply", "pong2"])
        .done()
      .build();

    const obj = world.objects.get("local:obj");
    expect(obj!.interface).toEqual(["ping"]);
    // Last handler wins
    expect(obj!.handlers.ping).toEqual(["perform", "reply", "pong2"]);
  });

  test("builds objects with prototype", () => {
    const world = new WorldBuilder()
      .object("proto:door")
        .handler("open", ["perform", "set", "open", true])
        .done()
      .object("local:door")
        .state("open", false)
        .prototype("proto:door")
        .done()
      .build();

    const door = world.objects.get("local:door");
    expect(door!.prototype).toBe("proto:door");
  });

  test("builds objects with children", () => {
    const world = new WorldBuilder()
      .object("local:room")
        .state("description", "A room.")
        .child("local:door")
        .child("local:chest")
        .done()
      .build();

    const room = world.objects.get("local:room");
    expect(room!.children).toEqual(["local:door", "local:chest"]);
  });

  test("builds multiple objects", () => {
    const world = new WorldBuilder()
      .object("local:a")
        .state("name", "A")
        .done()
      .object("local:b")
        .state("name", "B")
        .done()
      .object("local:c")
        .state("name", "C")
        .done()
      .build();

    expect(world.objects.size).toBe(3);
    expect(world.objects.get("local:a")!.state.name).toBe("A");
    expect(world.objects.get("local:b")!.state.name).toBe("B");
    expect(world.objects.get("local:c")!.state.name).toBe("C");
  });

  test("builds empty world", () => {
    const world = new WorldBuilder().build();
    expect(world.objects.size).toBe(0);
  });
});

describe("WorldBuilder + drain", () => {
  test("door + room example: look returns description", () => {
    const world = new WorldBuilder()
      .object("local:room")
        .state("description", "A dusty room.")
        .ref("door", "local:door")
        .handler("look", ["perform", "reply", ["get-in", ["get", "state"], "description"]])
        .done()
      .object("local:door")
        .state("open", false)
        .handler("open", ["perform", "set", "open", true])
        .done()
      .build();

    world.send("local:room", { verb: "look", payload: null });
    const replies = world.drain(100);
    expect(replies).toEqual(["A dusty room."]);
  });

  test("door + room: open door changes state", () => {
    const world = new WorldBuilder()
      .object("local:door")
        .state("open", false)
        .handler("open", [
          "do",
          ["perform", "set", "open", true],
          ["perform", "reply", "The door creaks open."],
        ])
        .done()
      .build();

    world.send("local:door", { verb: "open", payload: null });
    const replies = world.drain(100);

    expect(replies).toEqual(["The door creaks open."]);
    expect(world.objects.get("local:door")!.state.open).toBe(true);
  });

  test("cross-object messaging via refs", () => {
    const world = new WorldBuilder()
      .object("local:button")
        .ref("target", "local:lamp")
        .handler("press", [
          "perform", "send",
          ["get-in", ["get", "state"], "target"],
          "toggle", null,
        ])
        .done()
      .object("local:lamp")
        .state("on", false)
        .handler("toggle", [
          "if", ["get-in", ["get", "state"], "on"],
          ["perform", "set", "on", false],
          ["perform", "set", "on", true],
        ])
        .done()
      .build();

    world.send("local:button", { verb: "press", payload: null });
    world.drain(100);

    expect(world.objects.get("local:lamp")!.state.on).toBe(true);
  });

  test("prototype-based handler resolution", () => {
    const world = new WorldBuilder()
      .object("proto:container")
        .handler("look", [
          "perform", "reply",
          ["concat", "You see: ", ["get-in", ["get", "state"], "contents"]],
        ])
        .done()
      .object("local:chest")
        .state("contents", "gold coins")
        .prototype("proto:container")
        .done()
      .build();

    world.send("local:chest", { verb: "look", payload: null });
    const replies = world.drain(100);
    expect(replies).toEqual(["You see: gold coins"]);
  });

  test("attenuated ref blocks disallowed verbs", () => {
    const world = new WorldBuilder()
      .object("local:player")
        .attenuatedRef("door", "local:door", ["look"])
        .handler("try-open", [
          "perform", "send",
          ["get-in", ["get", "state"], "door"],
          "open", null,
        ])
        .done()
      .object("local:door")
        .state("open", false)
        .handler("open", ["perform", "set", "open", true])
        .handler("look", ["perform", "reply", "A wooden door."])
        .done()
      .build();

    // "open" should be blocked by the attenuated ref (only "look" allowed)
    world.send("local:player", { verb: "try-open", payload: null });
    world.drain(100);
    expect(world.objects.get("local:door")!.state.open).toBe(false);
  });
});
