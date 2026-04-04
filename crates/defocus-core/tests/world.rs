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

#[test]
fn prototype_basic_inheritance() {
    let mut world = World::new();

    let proto = Object::new("proto:greeter").with_handler(
        "greet",
        val(json!(["perform", "reply", ["get-in", ["get", "state"], "name"]])),
    );

    let instance = Object::new("local:instance")
        .with_state("name", "Alice")
        .with_prototype("proto:greeter");

    world.add(proto);
    world.add(instance);

    let replies = send(&mut world, "local:instance", "greet", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("Alice".into()));
}

#[test]
fn prototype_override() {
    let mut world = World::new();

    let proto = Object::new("proto:greeter").with_handler(
        "greet",
        val(json!(["perform", "reply", "proto hello"])),
    );

    let instance = Object::new("local:instance")
        .with_prototype("proto:greeter")
        .with_handler("greet", val(json!(["perform", "reply", "instance hello"])));

    world.add(proto);
    world.add(instance);

    let replies = send(&mut world, "local:instance", "greet", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("instance hello".into()));
}

#[test]
fn prototype_chain() {
    let mut world = World::new();

    let c = Object::new("proto:c").with_handler(
        "greet",
        val(json!(["perform", "reply", "from C"])),
    );
    let b = Object::new("proto:b").with_prototype("proto:c");
    let a = Object::new("local:a").with_prototype("proto:b");

    world.add(c);
    world.add(b);
    world.add(a);

    let replies = send(&mut world, "local:a", "greet", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("from C".into()));
}

#[test]
fn prototype_state_isolation() {
    let mut world = World::new();

    let proto = Object::new("proto:greeter")
        .with_state("name", "Proto")
        .with_handler(
            "greet",
            val(json!(["perform", "reply", ["get-in", ["get", "state"], "name"]])),
        );

    let instance = Object::new("local:instance")
        .with_state("name", "Instance")
        .with_prototype("proto:greeter");

    world.add(proto);
    world.add(instance);

    let replies = send(&mut world, "local:instance", "greet", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("Instance".into()));
}

#[test]
fn prototype_stub_with_prototype() {
    let mut world = World::new();

    let proto = Object::new("proto:greeter").with_handler(
        "greet",
        val(json!(["perform", "reply", "hello from proto"])),
    );

    let instance = Object::stub("local:instance", vec!["greet".into()])
        .with_prototype("proto:greeter");

    world.add(proto);
    world.add(instance);

    let replies = send(&mut world, "local:instance", "greet", Value::Null);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0], Value::String("hello from proto".into()));
}

#[test]
fn prototype_cycle_protection() {
    let mut world = World::new();

    let a = Object::new("local:a").with_prototype("local:b");
    let b = Object::new("local:b").with_prototype("local:a");

    world.add(a);
    world.add(b);

    // Should not infinite loop — neither has a handler for "greet"
    let replies = send(&mut world, "local:a", "greet", Value::Null);
    assert_eq!(replies.len(), 0);
}

// --- Event Log Tests ---

use defocus_core::log::EventLog;

fn make_counter_world() -> World {
    let mut world = World::new();
    let counter = Object::new("local:counter")
        .with_state("count", Value::Int(0))
        .with_handler(
            "increment",
            val(json!([
                "do",
                ["perform", "set", "count", ["+", ["get-in", ["get", "state"], "count"], 1]],
                ["perform", "reply", ["+", ["get-in", ["get", "state"], "count"], 1]]
            ])),
        );
    world.add(counter);
    world
}

#[test]
fn log_captures_events() {
    let mut world = make_counter_world();
    world.enable_logging();

    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);

    let log = world.take_log().unwrap();
    assert_eq!(log.events.len(), 3);

    assert_eq!(log.events[0].target, "local:counter");
    assert_eq!(log.events[0].message.verb, "increment");
    assert_eq!(log.events[0].sender, None);
    assert_eq!(log.events[0].replies, vec![Value::Int(1)]);

    assert_eq!(log.events[1].replies, vec![Value::Int(2)]);
    assert_eq!(log.events[2].replies, vec![Value::Int(3)]);

    // Log was taken — should be None now
    assert!(world.take_log().is_none());
}

#[test]
fn replay_produces_same_state() {
    // Run with logging
    let mut world = make_counter_world();
    let snapshot = world.clone();
    world.enable_logging();

    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);

    let log = world.take_log().unwrap();

    // Replay on a fresh copy
    let (replayed, _replies) = EventLog::replay_from(&snapshot, &log);

    assert_eq!(
        replayed.objects["local:counter"].state["count"],
        world.objects["local:counter"].state["count"]
    );
    assert_eq!(
        replayed.objects["local:counter"].state["count"],
        Value::Int(3)
    );
}

#[test]
fn branch_at_point() {
    let mut world = make_counter_world();
    let snapshot = world.clone();
    world.enable_logging();

    for _ in 0..5 {
        send(&mut world, "local:counter", "increment", Value::Null);
    }
    let log = world.take_log().unwrap();
    assert_eq!(log.events.len(), 5);

    // Branch at message 3
    let (branched, truncated_log) = snapshot.fork_at(&log, 3);

    assert_eq!(truncated_log.events.len(), 3);
    assert_eq!(
        branched.objects["local:counter"].state["count"],
        Value::Int(3)
    );
}

#[test]
fn branch_and_diverge() {
    let mut world = make_counter_world();

    // Also add a second object to make divergence interesting
    let npc = Object::new("local:npc")
        .with_state("mood", "neutral")
        .with_handler(
            "greet",
            val(json!([
                "do",
                ["perform", "set", "mood", ["get", "payload"]],
                ["perform", "reply", ["get", "payload"]]
            ])),
        );
    world.add(npc);

    let snapshot = world.clone();
    world.enable_logging();

    // Send 3 increment messages
    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);

    let log = world.take_log().unwrap();

    // Branch at message 3 (all messages) and send a different 4th message
    let (mut branched, _) = snapshot.fork_at(&log, 3);
    assert_eq!(
        branched.objects["local:counter"].state["count"],
        Value::Int(3)
    );

    // Diverge: send a greet to npc instead of another increment
    send(
        &mut branched,
        "local:npc",
        "greet",
        Value::String("happy".into()),
    );

    // Continue original: send a 4th increment
    send(&mut world, "local:counter", "increment", Value::Null);

    // Verify divergence
    assert_eq!(
        world.objects["local:counter"].state["count"],
        Value::Int(4)
    );
    assert_eq!(
        branched.objects["local:counter"].state["count"],
        Value::Int(3)
    );
    assert_eq!(
        branched.objects["local:npc"].state["mood"],
        Value::String("happy".into())
    );
    assert_eq!(
        world.objects["local:npc"].state["mood"],
        Value::String("neutral".into())
    );
}

#[test]
fn event_log_serialization_roundtrip() {
    let mut world = make_counter_world();
    world.enable_logging();

    send(&mut world, "local:counter", "increment", Value::Null);
    send(&mut world, "local:counter", "increment", Value::Null);

    let log = world.take_log().unwrap();
    let json = serde_json::to_string(&log).unwrap();
    let restored: EventLog = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.events.len(), 2);
    assert_eq!(restored.events[0].target, "local:counter");
    assert_eq!(restored.events[0].message.verb, "increment");
    assert_eq!(restored.events[0].replies, vec![Value::Int(1)]);
    assert_eq!(restored.events[1].replies, vec![Value::Int(2)]);
}

// --- Capability Attenuation Tests ---

#[test]
fn attenuated_ref_allows_permitted_verb() {
    let mut world = World::new();

    // Door has an attenuated ref to frame that only allows "door-opened"
    let door = Object::new("local:door")
        .with_attenuated_ref("frame", "local:frame", vec!["door-opened".into()])
        .with_handler(
            "open",
            val(json!([
                "do",
                ["perform", "set", "open", true],
                [
                    "perform",
                    "send",
                    ["get-in", ["get", "state"], "frame"],
                    "door-opened",
                    null
                ]
            ])),
        );

    let frame = Object::new("local:frame")
        .with_state("doorOpen", false)
        .with_handler(
            "door-opened",
            val(json!(["perform", "set", "doorOpen", true])),
        );

    world.add(door);
    world.add(frame);
    send(&mut world, "local:door", "open", Value::Null);

    assert_eq!(world.objects["local:door"].state["open"], Value::Bool(true));
    assert_eq!(
        world.objects["local:frame"].state["doorOpen"],
        Value::Bool(true)
    );
}

#[test]
fn attenuated_ref_blocks_forbidden_verb() {
    let mut world = World::new();

    // Door has an attenuated ref to frame that only allows "door-opened"
    let door = Object::new("local:door")
        .with_attenuated_ref("frame", "local:frame", vec!["door-opened".into()])
        .with_handler(
            "reset",
            val(json!([
                "perform",
                "send",
                ["get-in", ["get", "state"], "frame"],
                "destroy",
                null
            ])),
        );

    let frame = Object::new("local:frame")
        .with_state("destroyed", false)
        .with_handler(
            "destroy",
            val(json!(["perform", "set", "destroyed", true])),
        );

    world.add(door);
    world.add(frame);
    send(&mut world, "local:door", "reset", Value::Null);

    // "destroy" verb should be silently dropped — state unchanged
    assert_eq!(
        world.objects["local:frame"].state["destroyed"],
        Value::Bool(false)
    );
}

#[test]
fn attenuate_op_restricts_further() {
    let mut world = World::new();

    // Object starts with unrestricted ref, attenuates to ["open", "close"],
    // then further to ["open"]. Sends "close" → should be dropped.
    let obj = Object::new("local:obj")
        .with_ref("target", "local:target")
        .with_handler(
            "test",
            val(json!([
                "do",
                // First attenuate to open+close
                ["let", "narrow",
                    ["attenuate", ["get-in", ["get", "state"], "target"], ["array", "open", "close"]],
                    // Then attenuate further to just open
                    ["let", "narrower",
                        ["attenuate", ["get", "narrow"], ["array", "open"]],
                        // Try sending "close" via the narrower ref
                        ["do",
                            ["perform", "send", ["get", "narrower"], "close", null],
                            // Also send "open" — should succeed
                            ["perform", "send", ["get", "narrower"], "open", null]
                        ]
                    ]
                ]
            ])),
        );

    let target = Object::new("local:target")
        .with_state("opened", false)
        .with_state("closed", false)
        .with_handler(
            "open",
            val(json!(["perform", "set", "opened", true])),
        )
        .with_handler(
            "close",
            val(json!(["perform", "set", "closed", true])),
        );

    world.add(obj);
    world.add(target);
    send(&mut world, "local:obj", "test", Value::Null);

    // "close" should be dropped, "open" should succeed
    assert_eq!(
        world.objects["local:target"].state["opened"],
        Value::Bool(true)
    );
    assert_eq!(
        world.objects["local:target"].state["closed"],
        Value::Bool(false)
    );
}

#[test]
fn attenuated_ref_serialization_roundtrip() {
    let mut world = World::new();

    let obj = Object::new("local:obj")
        .with_attenuated_ref("target", "local:other", vec!["open".into(), "close".into()]);

    world.add(obj);

    let json = world.to_json();

    // Verify the attenuated ref serialization
    let state = &json["objects"]["local:obj"]["state"]["target"];
    assert_eq!(state["$ref"], "local:other");
    assert_eq!(
        state["$verbs"],
        serde_json::json!(["open", "close"])
    );

    // Round-trip
    let restored = World::from_json(json).unwrap();
    let restored_obj = restored.objects.get("local:obj").unwrap();
    assert_eq!(
        restored_obj.state.get("target"),
        Some(&Value::Ref {
            id: "local:other".into(),
            verbs: Some(vec!["open".into(), "close".into()]),
        })
    );
}

#[test]
fn unrestricted_ref_still_works() {
    // This is a backward-compat sanity check — unrestricted ref with no verbs
    let mut world = World::new();

    let door = Object::new("local:door")
        .with_ref("frame", "local:frame")
        .with_handler(
            "open",
            val(json!([
                "do",
                ["perform", "set", "open", true],
                [
                    "perform",
                    "send",
                    ["get-in", ["get", "state"], "frame"],
                    "door-opened",
                    null
                ]
            ])),
        );

    let frame = Object::new("local:frame")
        .with_state("doorOpen", false)
        .with_handler(
            "door-opened",
            val(json!(["perform", "set", "doorOpen", true])),
        );

    world.add(door);
    world.add(frame);
    send(&mut world, "local:door", "open", Value::Null);

    assert_eq!(world.objects["local:door"].state["open"], Value::Bool(true));
    assert_eq!(
        world.objects["local:frame"].state["doorOpen"],
        Value::Bool(true)
    );
}

// --- Scheduler Tests ---

#[test]
fn schedule_and_advance() {
    let mut world = World::new();

    let obj = Object::new("local:obj")
        .with_state("received", false)
        .with_handler(
            "ping",
            val(json!(["perform", "set", "received", true])),
        );

    world.add(obj);

    // Schedule a message at tick 5
    world.schedule.entry(5).or_default().push((
        "local:obj".into(),
        Message {
            verb: "ping".into(),
            payload: Value::Null,
        },
    ));

    // Advance to tick 4 — not yet delivered
    world.advance(4);
    assert_eq!(
        world.objects["local:obj"].state["received"],
        Value::Bool(false)
    );

    // Advance to tick 5 — delivered
    world.advance(5);
    assert_eq!(
        world.objects["local:obj"].state["received"],
        Value::Bool(true)
    );
}

#[test]
fn schedule_multiple_ticks() {
    let mut world = World::new();

    let obj = Object::new("local:obj")
        .with_state("count", Value::Int(0))
        .with_handler(
            "bump",
            val(json!([
                "perform", "set", "count",
                ["+", ["get-in", ["get", "state"], "count"], 1]
            ])),
        );

    world.add(obj);

    // Schedule at tick 3 and tick 7
    world.schedule.entry(3).or_default().push((
        "local:obj".into(),
        Message {
            verb: "bump".into(),
            payload: Value::Null,
        },
    ));
    world.schedule.entry(7).or_default().push((
        "local:obj".into(),
        Message {
            verb: "bump".into(),
            payload: Value::Null,
        },
    ));

    // Advance to 5 — only tick 3 fires
    world.advance(5);
    assert_eq!(world.objects["local:obj"].state["count"], Value::Int(1));

    // Advance to 10 — tick 7 fires
    world.advance(10);
    assert_eq!(world.objects["local:obj"].state["count"], Value::Int(2));
}

#[test]
fn schedule_from_handler() {
    let mut world = World::new();

    // Object A schedules a delayed message to object B
    let a = Object::new("local:a").with_handler(
        "trigger",
        val(json!([
            "perform", "schedule", 5, "local:b", "ping", null
        ])),
    );

    let b = Object::new("local:b")
        .with_state("pinged", false)
        .with_handler(
            "ping",
            val(json!(["perform", "set", "pinged", true])),
        );

    world.add(a);
    world.add(b);

    // Send trigger — this schedules the message, doesn't deliver it yet
    send(&mut world, "local:a", "trigger", Value::Null);
    assert_eq!(
        world.objects["local:b"].state["pinged"],
        Value::Bool(false)
    );

    // Advance to tick 5 — now it fires
    world.advance(5);
    assert_eq!(
        world.objects["local:b"].state["pinged"],
        Value::Bool(true)
    );
}

#[test]
fn advance_without_scheduled_messages() {
    let mut world = World::new();

    let obj = Object::new("local:obj").with_state("x", Value::Int(0));

    world.add(obj);

    // Advance without anything scheduled
    let replies = world.advance(10);
    assert!(replies.is_empty());
    assert_eq!(world.tick, 10);
}

#[test]
fn tick_persists_in_serialization() {
    let mut world = World::new();
    world.add(Object::new("local:obj"));

    // Set tick to 42 and schedule a message at tick 50
    world.tick = 42;
    world.schedule.entry(50).or_default().push((
        "local:obj".into(),
        Message {
            verb: "ping".into(),
            payload: Value::Null,
        },
    ));

    let json = world.to_json();

    // Verify tick is in the JSON
    assert_eq!(json["tick"], 42);
    assert!(json["schedule"].is_object());

    // Round-trip
    let restored = World::from_json(json).unwrap();
    assert_eq!(restored.tick, 42);
    assert_eq!(restored.schedule.len(), 1);
    assert!(restored.schedule.contains_key(&50));
    let msgs = &restored.schedule[&50];
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].0, "local:obj");
    assert_eq!(msgs[0].1.verb, "ping");
}
