use defocus_core::value::Value;
use defocus_core::world::{Message, Object, World};
use serde_json::json;

fn val(j: serde_json::Value) -> Value {
    serde_json::from_value(j).unwrap()
}

fn send(world: &mut World, to: &str, verb: &str, payload: Value) -> Vec<Value> {
    world.send(
        to.into(),
        Message {
            verb: verb.into(),
            payload,
        },
    );
    world.drain(100)
}

#[test]
fn door_opens_and_notifies_frame() {
    let mut world = World::new();

    let door = Object::new("local:door")
        .with_state("open", false)
        .with_handler(
            "open",
            val(json!([
                "do",
                ["perform", "set", "open", true],
                ["perform", "send", "local:frame", "door-opened", null]
            ])),
        )
        .with_handler("close", val(json!(["perform", "set", "open", false])));

    let frame = Object::new("local:frame")
        .with_state("doorOpen", false)
        .with_handler(
            "door-opened",
            val(json!(["perform", "set", "doorOpen", true])),
        );

    world.add(door);
    world.add(frame);
    world.send(
        "local:door".into(),
        Message {
            verb: "open".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert_eq!(world.objects["local:door"].state["open"], Value::Bool(true));
    assert_eq!(
        world.objects["local:frame"].state["doorOpen"],
        Value::Bool(true)
    );
}

#[test]
fn conditional_handler_based_on_state() {
    let mut world = World::new();

    let light = Object::new("local:light")
        .with_state("on", false)
        .with_handler(
            "toggle",
            val(json!([
                "if",
                ["get-in", ["get", "state"], "on"],
                ["perform", "set", "on", false],
                ["perform", "set", "on", true]
            ])),
        );

    world.add(light);

    world.send(
        "local:light".into(),
        Message {
            verb: "toggle".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);
    assert_eq!(world.objects["local:light"].state["on"], Value::Bool(true));

    world.send(
        "local:light".into(),
        Message {
            verb: "toggle".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);
    assert_eq!(world.objects["local:light"].state["on"], Value::Bool(false));
}

#[test]
fn pattern_matching_on_payload() {
    let mut world = World::new();

    let npc = Object::new("local:npc")
        .with_state("mood", "neutral")
        .with_handler(
            "greet",
            val(json!([
                "match",
                ["get", "payload"],
                ["friendly", ["perform", "set", "mood", "happy"]],
                ["hostile", ["perform", "set", "mood", "angry"]],
                ["_", ["perform", "set", "mood", "confused"]]
            ])),
        );

    world.add(npc);

    world.send(
        "local:npc".into(),
        Message {
            verb: "greet".into(),
            payload: Value::String("friendly".into()),
        },
    );
    world.drain(100);
    assert_eq!(
        world.objects["local:npc"].state["mood"],
        Value::String("happy".into())
    );

    world.send(
        "local:npc".into(),
        Message {
            verb: "greet".into(),
            payload: Value::String("unknown".into()),
        },
    );
    world.drain(100);
    assert_eq!(
        world.objects["local:npc"].state["mood"],
        Value::String("confused".into())
    );
}

#[test]
fn stub_objects_satisfy_interface_without_handlers() {
    let mut world = World::new();

    let server = Object::stub("local:server", vec!["ping".into(), "query".into()]);
    world.add(server);

    world.send(
        "local:server".into(),
        Message {
            verb: "ping".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert!(world.objects.contains_key("local:server"));
    assert_eq!(
        world.objects["local:server"].interface,
        vec!["ping".to_string(), "query".to_string()]
    );
}

#[test]
fn counter_increments() {
    let mut world = World::new();

    let factory = Object::new("local:factory")
        .with_state("count", Value::Int(0))
        .with_handler(
            "produce",
            val(json!([
                "perform",
                "set",
                "count",
                ["+", ["get-in", ["get", "state"], "count"], 1]
            ])),
        );

    world.add(factory);
    world.send(
        "local:factory".into(),
        Message {
            verb: "produce".into(),
            payload: Value::Null,
        },
    );
    world.send(
        "local:factory".into(),
        Message {
            verb: "produce".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert_eq!(world.objects["local:factory"].state["count"], Value::Int(2));
}

#[test]
fn spawn_creates_object_in_world() {
    let mut world = World::new();

    let spawner = Object::new("local:spawner").with_handler(
        "create",
        val(json!([
            "perform",
            "spawn",
            "local:child",
            {
                "state": { "alive": true },
                "handlers": {
                    "ping": ["perform", "set", "ponged", true]
                },
                "interface": ["ping"]
            }
        ])),
    );

    world.add(spawner);
    world.send(
        "local:spawner".into(),
        Message {
            verb: "create".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert!(world.objects.contains_key("local:child"));
    let child = &world.objects["local:child"];
    assert_eq!(child.state["alive"], Value::Bool(true));
    assert_eq!(child.interface, vec!["ping".to_string()]);
    assert!(child.handlers.contains_key("ping"));
}

#[test]
fn spawn_returns_ref_usable_for_send() {
    let mut world = World::new();

    // Spawner creates a child and immediately sends it a message via the returned ref
    let spawner = Object::new("local:spawner").with_handler(
        "create-and-ping",
        val(json!([
            "let",
            "child-ref",
            [
                "perform",
                "spawn",
                "local:child",
                {
                    "state": { "pinged": false },
                    "handlers": {
                        "ping": ["perform", "set", "pinged", true]
                    },
                    "interface": ["ping"]
                }
            ],
            ["perform", "send", ["get", "child-ref"], "ping", null]
        ])),
    );

    world.add(spawner);
    world.send(
        "local:spawner".into(),
        Message {
            verb: "create-and-ping".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert!(world.objects.contains_key("local:child"));
    assert_eq!(
        world.objects["local:child"].state["pinged"],
        Value::Bool(true)
    );
}

#[test]
fn spawned_object_handlers_work() {
    let mut world = World::new();

    let spawner = Object::new("local:spawner").with_handler(
        "create",
        val(json!([
            "perform",
            "spawn",
            "local:counter",
            {
                "state": { "count": 0 },
                "handlers": {
                    "increment": [
                        "perform",
                        "set",
                        "count",
                        ["+", ["get-in", ["get", "state"], "count"], 1]
                    ]
                },
                "interface": ["increment"]
            }
        ])),
    );

    world.add(spawner);
    world.send(
        "local:spawner".into(),
        Message {
            verb: "create".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    // Now send messages directly to the spawned object
    world.send(
        "local:counter".into(),
        Message {
            verb: "increment".into(),
            payload: Value::Null,
        },
    );
    world.send(
        "local:counter".into(),
        Message {
            verb: "increment".into(),
            payload: Value::Null,
        },
    );
    world.send(
        "local:counter".into(),
        Message {
            verb: "increment".into(),
            payload: Value::Null,
        },
    );
    world.drain(100);

    assert_eq!(world.objects["local:counter"].state["count"], Value::Int(3));
}

#[test]
fn interactive_fiction_scenario() {
    let mut world = World::new();

    // Room — contains refs to door and npc, replies with description
    let room = Object::new("local:room")
        .with_state(
            "description",
            "A dimly lit stone chamber. A heavy wooden door stands to the north. An old woman sits in the corner.",
        )
        .with_ref("door", "local:door")
        .with_ref("npc", "local:npc")
        .with_handler(
            "look",
            val(json!([
                "do",
                ["perform", "reply", ["get-in", ["get", "state"], "description"]],
                ["perform", "reply",
                    ["concat",
                        "Door: ",
                        ["if",
                            ["get-in", ["get", "state"], "door"],
                            "There is a door to the north.",
                            "No door here."
                        ]
                    ]
                ],
                ["perform", "reply",
                    ["concat",
                        "NPC: ",
                        ["if",
                            ["get-in", ["get", "state"], "npc"],
                            "An old woman sits in the corner.",
                            "Nobody is here."
                        ]
                    ]
                ]
            ])),
        );

    // Door — open/close/look with state
    let door = Object::new("local:door")
        .with_state("open", false)
        .with_state(
            "description",
            "A heavy wooden door, reinforced with iron bands.",
        )
        .with_handler(
            "look",
            val(json!([
                "perform",
                "reply",
                [
                    "concat",
                    ["get-in", ["get", "state"], "description"],
                    " It is ",
                    ["if", ["get-in", ["get", "state"], "open"], "open", "closed"],
                    "."
                ]
            ])),
        )
        .with_handler(
            "open",
            val(json!([
                "if",
                ["get-in", ["get", "state"], "open"],
                ["perform", "reply", "The door is already open."],
                [
                    "do",
                    ["perform", "set", "open", true],
                    ["perform", "reply", "The door creaks open."]
                ]
            ])),
        )
        .with_handler(
            "close",
            val(json!([
                "if",
                ["not", ["get-in", ["get", "state"], "open"]],
                ["perform", "reply", "The door is already closed."],
                [
                    "do",
                    ["perform", "set", "open", false],
                    ["perform", "reply", "The door swings shut."]
                ]
            ])),
        );

    // NPC — look/talk with pattern matching
    let npc = Object::new("local:npc")
        .with_state("name", "Old Woman")
        .with_state("mood", "wary")
        .with_state(
            "description",
            "An old woman with sharp eyes watches you carefully.",
        )
        .with_handler(
            "look",
            val(json!([
                "perform",
                "reply",
                ["get-in", ["get", "state"], "description"]
            ])),
        )
        .with_handler(
            "talk",
            val(json!([
                "match",
                ["get", "payload"],
                [
                    "greeting",
                    [
                        "do",
                        ["perform", "set", "mood", "warm"],
                        ["perform", "reply", "She nods slowly. 'Welcome, traveler.'"]
                    ]
                ],
                [
                    "threat",
                    [
                        "do",
                        ["perform", "set", "mood", "hostile"],
                        [
                            "perform",
                            "reply",
                            "Her eyes narrow. 'You'd best move along.'"
                        ]
                    ]
                ],
                ["_", ["perform", "reply", "She regards you silently."]]
            ])),
        );

    world.add(room);
    world.add(door);
    world.add(npc);

    // 1. Look at room
    let replies = send(&mut world, "local:room", "look", Value::Null);
    assert_eq!(replies.len(), 3);
    assert_eq!(
        replies[0],
        Value::String("A dimly lit stone chamber. A heavy wooden door stands to the north. An old woman sits in the corner.".into())
    );
    assert!(replies[1].as_str().unwrap().contains("door"));
    let npc_line = replies[2].as_str().unwrap();
    assert!(
        npc_line.contains("old woman") || npc_line.contains("NPC"),
        "expected NPC mention in: {npc_line}"
    );

    // 2. Look at door — verify mentions "closed"
    let replies = send(&mut world, "local:door", "look", Value::Null);
    assert_eq!(replies.len(), 1);
    let door_desc = replies[0].as_str().unwrap();
    assert!(
        door_desc.contains("closed"),
        "expected 'closed' in: {door_desc}"
    );

    // 3. Open door
    let replies = send(&mut world, "local:door", "open", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("The door creaks open.".into()));
    assert_eq!(world.objects["local:door"].state["open"], Value::Bool(true));

    // 4. Open door again
    let replies = send(&mut world, "local:door", "open", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0],
        Value::String("The door is already open.".into())
    );

    // 5. Talk to NPC — greeting
    let replies = send(
        &mut world,
        "local:npc",
        "talk",
        Value::String("greeting".into()),
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0],
        Value::String("She nods slowly. 'Welcome, traveler.'".into())
    );
    assert_eq!(
        world.objects["local:npc"].state["mood"],
        Value::String("warm".into())
    );

    // 6. Talk to NPC — threat
    let replies = send(
        &mut world,
        "local:npc",
        "talk",
        Value::String("threat".into()),
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0],
        Value::String("Her eyes narrow. 'You'd best move along.'".into())
    );
    assert_eq!(
        world.objects["local:npc"].state["mood"],
        Value::String("hostile".into())
    );

    // 7. Talk to NPC — wildcard
    let replies = send(
        &mut world,
        "local:npc",
        "talk",
        Value::String("weather".into()),
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0],
        Value::String("She regards you silently.".into())
    );
}
