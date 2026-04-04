use defocus_core::value::Value;
use defocus_core::world::{Message, Object, World};
use serde_json::json;

fn val(j: serde_json::Value) -> Value {
    serde_json::from_value(j).unwrap()
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
