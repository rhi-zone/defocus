import { describe, test, expect } from "bun:test";
import { World, createObject } from "./world.js";
import { MockProvider } from "./llm.js";
import type { DefocusObject } from "./world.js";
import type { Value } from "./value.js";

function sendMsg(world: World, to: string, verb: string, payload: Value = null): Value[] {
  world.send(to, { verb, payload });
  return world.drain(100);
}

describe("MockProvider", () => {
  test("returns default response when no needle matches", () => {
    const provider = new MockProvider("I don't know.");
    expect(provider.complete("anything")).toBe("I don't know.");
  });

  test("returns matched response", () => {
    const provider = new MockProvider("default")
      .withResponse("hello", "Hi there!")
      .withResponse("bye", "Goodbye!");
    expect(provider.complete("say hello to me")).toBe("Hi there!");
    expect(provider.complete("time to say bye")).toBe("Goodbye!");
    expect(provider.complete("something else")).toBe("default");
  });
});

describe("LLM integration", () => {
  test("LLM call returns response and handler uses it in reply", () => {
    const world = new World();
    world.llm = new MockProvider("I don't understand.")
      .withResponse("hello", "Greetings, traveler!");

    const npc: DefocusObject = {
      id: "local:npc",
      state: {},
      handlers: {
        talk: [
          "let", "response",
          ["llm", ["concat", "The player says: ", ["get", "payload"]]],
          ["do",
            ["perform", "set", "last-response", ["get", "response"]],
            ["perform", "reply", ["get", "response"]],
          ],
        ],
      },
      interface: ["talk"],
      children: [],
      prototype: null,
    };
    world.add(npc);

    const replies = sendMsg(world, "local:npc", "talk", "hello");
    expect(replies).toEqual(["Greetings, traveler!"]);
    expect(world.objects.get("local:npc")!.state["last-response"]).toBe("Greetings, traveler!");
  });

  test("no LLM provider returns null, handler falls back", () => {
    const world = new World();
    // No LLM provider set

    const npc: DefocusObject = {
      id: "local:npc",
      state: {},
      handlers: {
        talk: [
          "let", "response", ["llm", "anything"],
          ["if", ["get", "response"],
            ["perform", "reply", ["get", "response"]],
            ["perform", "reply", "I have nothing to say."],
          ],
        ],
      },
      interface: ["talk"],
      children: [],
      prototype: null,
    };
    world.add(npc);

    const replies = sendMsg(world, "local:npc", "talk");
    expect(replies).toEqual(["I have nothing to say."]);
  });

  test("LLM-driven NPC stores state across interactions", () => {
    const world = new World();
    world.llm = new MockProvider("*silence*")
      .withResponse("greet", "Welcome to my shop!")
      .withResponse("buy", "That'll be 10 gold.");

    const npc: DefocusObject = {
      id: "local:shopkeeper",
      state: { "interaction-count": 0 },
      handlers: {
        talk: [
          "let", "response",
          ["llm", ["concat", "Action: ", ["get", "payload"]]],
          ["do",
            ["perform", "set", "last-response", ["get", "response"]],
            ["perform", "set", "interaction-count",
              ["+", ["get-in", ["get", "state"], "interaction-count"], 1]],
            ["perform", "reply", ["get", "response"]],
          ],
        ],
      },
      interface: ["talk"],
      children: [],
      prototype: null,
    };
    world.add(npc);

    // First interaction
    let replies = sendMsg(world, "local:shopkeeper", "talk", "greet");
    expect(replies).toEqual(["Welcome to my shop!"]);

    // Second interaction
    replies = sendMsg(world, "local:shopkeeper", "talk", "buy sword");
    expect(replies).toEqual(["That'll be 10 gold."]);

    const shopkeeper = world.objects.get("local:shopkeeper")!;
    expect(shopkeeper.state["interaction-count"]).toBe(2);
    expect(shopkeeper.state["last-response"]).toBe("That'll be 10 gold.");
  });
});
