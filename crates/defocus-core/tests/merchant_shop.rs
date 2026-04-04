//! The Merchant's Shop — comprehensive integration test exercising all defocus features:
//! prototypes, attenuated refs, pattern matching, scheduling, user-defined functions,
//! event logging, branching/forking, and save/load via MemoryBackend.

use defocus_core::persist::{MemoryBackend, SaveBackend};
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
    world.drain(1000)
}

fn build_world() -> World {
    let mut world = World::new();

    // -- item-prototype: shared handler for all items --
    // "look" → reply with the item's own description (from its state, not the prototype's)
    // "take" → reply "You pick up the [name]." + set location to sender ref
    let item_prototype = Object::new("item-prototype")
        .with_handler(
            "look",
            val(json!(["perform", "reply", ["get-in", ["get", "state"], "description"]])),
        )
        .with_handler(
            "take",
            val(json!([
                "do",
                ["perform", "reply",
                    ["concat", "You pick up the ", ["get-in", ["get", "state"], "name"], "."]],
                ["perform", "set", "location", ["get", "sender"]]
            ])),
        );
    world.add(item_prototype);

    // -- local:shop — the room --
    // State holds refs to merchant, key (attenuated), door (attenuated)
    // Handlers: look (replies description + sends look to contained objects), enter
    let shop = Object::new("local:shop")
        .with_state(
            "description",
            "A cluttered merchant's shop. Shelves line every wall.",
        )
        .with_ref("merchant", "local:merchant")
        .with_attenuated_ref("key", "local:key", vec!["look".into(), "take".into()])
        .with_attenuated_ref("door", "local:door", vec!["look".into(), "unlock".into()])
        .with_handler(
            "look",
            val(json!([
                "do",
                ["perform", "reply", ["get-in", ["get", "state"], "description"]],
                ["perform", "send", ["get-in", ["get", "state"], "merchant"], "look", null],
                ["perform", "send", ["get-in", ["get", "state"], "key"], "look", null],
                ["perform", "send", ["get-in", ["get", "state"], "door"], "look", null]
            ])),
        )
        .with_handler(
            "enter",
            val(json!(["perform", "reply", "You step inside the shop."])),
        );
    world.add(shop);

    // -- local:merchant — NPC --
    // Handlers: look, talk (pattern match on payload)
    let merchant = Object::new("local:merchant")
        .with_state("name", "Grizzled Merchant")
        .with_state("mood", "suspicious")
        .with_state(
            "description",
            "A weathered merchant eyes you from behind the counter.",
        )
        .with_handler(
            "look",
            val(json!(["perform", "reply", ["get-in", ["get", "state"], "description"]])),
        )
        .with_handler(
            "talk",
            val(json!([
                "match", ["get", "payload"],
                ["buy", ["do",
                    ["perform", "reply", "What are you buying?"],
                    ["perform", "set", "mood", "interested"]
                ]],
                ["haggle", ["if",
                    ["=", ["get-in", ["get", "state"], "mood"], "interested"],
                    ["do",
                        ["perform", "set", "mood", "amused"],
                        ["perform", "reply", "Ha! I like your spirit."]
                    ],
                    ["perform", "reply", "Buy something first."]
                ]],
                ["_", ["perform", "reply", "The merchant grunts."]]
            ])),
        );
    world.add(merchant);

    // -- local:key — inherits from item-prototype --
    let key = Object::new("local:key")
        .with_prototype("item-prototype")
        .with_state("name", "Brass Key")
        .with_state("description", "A tarnished brass key.");
    world.add(key);

    // -- local:door — stateful with scheduled effect --
    let door = Object::new("local:door")
        .with_state("locked", Value::Bool(true))
        .with_state("open", Value::Bool(false))
        .with_state("description", "A heavy oak door with an iron lock.")
        .with_handler(
            "look",
            // User-defined function: status-desc generates a combined status string
            val(json!([
                "let", "status-desc",
                ["fn", ["locked", "open"], [
                    "concat",
                    ["if", ["get", "locked"], "Locked", "Unlocked"],
                    " and ",
                    ["if", ["get", "open"], "open", "closed"],
                    "."
                ]],
                ["perform", "reply",
                    ["concat",
                        ["get-in", ["get", "state"], "description"],
                        " ",
                        ["call", ["get", "status-desc"],
                            ["get-in", ["get", "state"], "locked"],
                            ["get-in", ["get", "state"], "open"]
                        ]
                    ]
                ]
            ])),
        )
        .with_handler(
            "unlock",
            val(json!([
                "if", ["get-in", ["get", "state"], "locked"],
                ["do",
                    ["perform", "set", "locked", false],
                    ["perform", "reply", "Click. The lock turns."],
                    ["perform", "schedule", 3, ["get", "self"], "creak", null]
                ],
                ["perform", "reply", "Already unlocked."]
            ])),
        )
        .with_handler(
            "creak",
            val(json!(["perform", "reply", "The door creaks ominously..."])),
        )
        .with_handler(
            "open",
            val(json!([
                "if", ["get-in", ["get", "state"], "locked"],
                ["perform", "reply", "The door is locked."],
                ["if", ["get-in", ["get", "state"], "open"],
                    ["perform", "reply", "Already open."],
                    ["do",
                        ["perform", "set", "open", true],
                        ["perform", "reply", "The door swings open."]
                    ]
                ]
            ])),
        );
    world.add(door);

    world
}

#[test]
fn merchant_shop_full_scenario() {
    let mut world = build_world();

    // 1. Enter the shop
    let replies = send(&mut world, "local:shop", "enter", Value::Null);
    assert_eq!(replies, vec![Value::String("You step inside the shop.".into())]);

    // 2. Look around — shop replies with description, then sends look to merchant/key/door
    let replies = send(&mut world, "local:shop", "look", Value::Null);
    // First reply: room description (from shop handler's perform reply)
    assert_eq!(
        replies[0],
        Value::String("A cluttered merchant's shop. Shelves line every wall.".into())
    );
    // The shop sends look to merchant, key, and door — those produce replies too
    assert!(replies.len() >= 2);
    // Merchant look reply
    assert_eq!(
        replies[1],
        Value::String("A weathered merchant eyes you from behind the counter.".into())
    );
    // Key look reply (via prototype handler)
    assert_eq!(
        replies[2],
        Value::String("A tarnished brass key.".into())
    );
    // Door look reply (via user-defined function)
    assert_eq!(
        replies[3],
        Value::String(
            "A heavy oak door with an iron lock. Locked and closed.".into()
        )
    );

    // 3. Examine the key directly via attenuated ref (only look and take allowed)
    let replies = send(&mut world, "local:key", "look", Value::Null);
    assert_eq!(replies, vec![Value::String("A tarnished brass key.".into())]);

    // 4. Try to open the door (locked)
    let replies = send(&mut world, "local:door", "open", Value::Null);
    assert_eq!(
        replies,
        vec![Value::String("The door is locked.".into())]
    );

    // 5. Take the key (prototype handler fires with key's state)
    let replies = send(&mut world, "local:key", "take", Value::Null);
    assert_eq!(
        replies,
        vec![Value::String("You pick up the Brass Key.".into())]
    );
    // Key's location should be set to sender (null for external sends)
    assert_eq!(
        world.objects.get("local:key").unwrap().state.get("location"),
        Some(&Value::Null)
    );

    // 6. Talk to merchant "buy" — verify reply + mood change
    let replies = send(
        &mut world,
        "local:merchant",
        "talk",
        Value::String("buy".into()),
    );
    assert_eq!(
        replies,
        vec![Value::String("What are you buying?".into())]
    );
    assert_eq!(
        world
            .objects
            .get("local:merchant")
            .unwrap()
            .state
            .get("mood"),
        Some(&Value::String("interested".into()))
    );

    // 7. Talk to merchant "haggle" — mood is now "interested" so this works
    let replies = send(
        &mut world,
        "local:merchant",
        "talk",
        Value::String("haggle".into()),
    );
    assert_eq!(
        replies,
        vec![Value::String("Ha! I like your spirit.".into())]
    );
    assert_eq!(
        world
            .objects
            .get("local:merchant")
            .unwrap()
            .state
            .get("mood"),
        Some(&Value::String("amused".into()))
    );

    // 8. Unlock the door — verify reply + scheduled creak at tick 3
    let replies = send(&mut world, "local:door", "unlock", Value::Null);
    assert_eq!(
        replies,
        vec![Value::String("Click. The lock turns.".into())]
    );
    assert_eq!(
        world.objects.get("local:door").unwrap().state.get("locked"),
        Some(&Value::Bool(false))
    );
    // A message should be scheduled at tick 3
    assert!(world.schedule.contains_key(&3));

    // 9. Advance to tick 3 — creak fires
    let replies = world.advance(3);
    assert_eq!(
        replies,
        vec![Value::String("The door creaks ominously...".into())]
    );

    // 10. Open the door — now unlocked, should succeed
    let replies = send(&mut world, "local:door", "open", Value::Null);
    assert_eq!(
        replies,
        vec![Value::String("The door swings open.".into())]
    );
    assert_eq!(
        world.objects.get("local:door").unwrap().state.get("open"),
        Some(&Value::Bool(true))
    );

    // 11. Enable logging, talk to merchant again
    world.enable_logging();
    let replies = send(
        &mut world,
        "local:merchant",
        "talk",
        Value::String("haggle".into()),
    );
    // Mood is "amused" now, not "interested", so haggle fails
    assert_eq!(
        replies,
        vec![Value::String("Buy something first.".into())]
    );

    // Do one more action to have multiple events in the log
    let _replies = send(&mut world, "local:merchant", "talk", Value::Null);

    let log = world.take_log().unwrap();
    assert_eq!(log.events.len(), 2);
    assert_eq!(log.events[0].target, "local:merchant");
    assert_eq!(log.events[0].message.verb, "talk");
    assert_eq!(
        log.events[0].replies,
        vec![Value::String("Buy something first.".into())]
    );
    assert_eq!(log.events[1].target, "local:merchant");
    assert_eq!(
        log.events[1].replies,
        vec![Value::String("The merchant grunts.".into())]
    );

    // 12. Save world to MemoryBackend
    let mut backend = MemoryBackend::default();
    world.save_to(&mut backend, "merchant-shop");
    assert!(backend.list().contains(&"merchant-shop".to_string()));

    // 13. Branch the log — fork at the midpoint (after first event)
    // We need a snapshot from before logging started; we can use save/load for that
    // Instead, let's re-enable logging, do two actions, then fork at index 1
    world.enable_logging();
    let _r1 = send(
        &mut world,
        "local:merchant",
        "talk",
        Value::String("buy".into()),
    );
    let _r2 = send(&mut world, "local:door", "look", Value::Null);

    let log = world.take_log().unwrap();
    assert_eq!(log.events.len(), 2);

    // Save current state as base for fork
    let mut fork_backend = MemoryBackend::default();
    // We need the pre-log state, so load from our earlier save
    let base_world = World::load_from(&backend, "merchant-shop")
        .unwrap()
        .unwrap();

    // Fork at event index 1 (only replay the first event, skip the second)
    let (forked_world, truncated_log) = base_world.fork_at(&log, 1);
    assert_eq!(truncated_log.events.len(), 1);

    // 14. In the branch, take a different action — verify divergence
    let mut forked = forked_world;
    let forked_replies = send(
        &mut forked,
        "local:merchant",
        "talk",
        Value::String("haggle".into()),
    );
    // In the forked world, after the "buy" event, mood is "interested"
    // so haggle should succeed
    assert_eq!(
        forked_replies,
        vec![Value::String("Ha! I like your spirit.".into())]
    );
    assert_eq!(
        forked
            .objects
            .get("local:merchant")
            .unwrap()
            .state
            .get("mood"),
        Some(&Value::String("amused".into()))
    );

    // In the original world, merchant mood is "interested" (from the "buy" we did)
    assert_eq!(
        world
            .objects
            .get("local:merchant")
            .unwrap()
            .state
            .get("mood"),
        Some(&Value::String("interested".into()))
    );

    // 15. Load the saved world — verify state matches pre-save
    let loaded = World::load_from(&backend, "merchant-shop")
        .unwrap()
        .unwrap();
    // Door should be open and unlocked
    assert_eq!(
        loaded.objects.get("local:door").unwrap().state.get("open"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        loaded
            .objects
            .get("local:door")
            .unwrap()
            .state
            .get("locked"),
        Some(&Value::Bool(false))
    );
    // Merchant mood should be "amused" (the state when we saved)
    assert_eq!(
        loaded
            .objects
            .get("local:merchant")
            .unwrap()
            .state
            .get("mood"),
        Some(&Value::String("amused".into()))
    );
    // Key should still exist with its prototype
    assert_eq!(
        loaded.objects.get("local:key").unwrap().prototype,
        Some("item-prototype".into())
    );
    // Tick should be 3 (from the advance)
    assert_eq!(loaded.tick, 3);
    // All 5 objects should be present
    assert_eq!(loaded.objects.len(), 5);

    // Verify the loaded world is functional — door look still uses fn
    fork_backend.save("loaded-shop", &serde_json::to_string(&loaded.to_json()).unwrap());
    let mut loaded = loaded;
    let replies = send(&mut loaded, "local:door", "look", Value::Null);
    assert_eq!(
        replies,
        vec![Value::String(
            "A heavy oak door with an iron lock. Unlocked and open.".into()
        )]
    );
}

/// Verify attenuated refs on the shop prevent forbidden verbs.
#[test]
fn shop_attenuated_refs_block_forbidden_verbs() {
    let mut world = build_world();

    // The shop's door ref is attenuated to ["look", "unlock"].
    // If the shop tries to send "open" to the door via its ref, it should be silently dropped.
    // We can test this by adding a handler that sends "open" via the attenuated ref.
    let shop = world.objects.get_mut("local:shop").unwrap();
    shop.handlers.insert(
        "try-open-door".into(),
        val(json!([
            "perform", "send", ["get-in", ["get", "state"], "door"], "open", null
        ])),
    );
    shop.interface.push("try-open-door".into());

    // First unlock the door so "open" would succeed if it got through
    send(&mut world, "local:door", "unlock", Value::Null);

    // Now try to open door through the attenuated ref
    send(&mut world, "local:shop", "try-open-door", Value::Null);

    // Door should still be closed — the "open" verb was not in the allowed list
    assert_eq!(
        world.objects.get("local:door").unwrap().state.get("open"),
        Some(&Value::Bool(false))
    );

    // But "unlock" via the attenuated ref would work (already unlocked though)
    // Verify "look" works through the ref
    let shop = world.objects.get_mut("local:shop").unwrap();
    shop.handlers.insert(
        "look-at-door".into(),
        val(json!([
            "perform", "send", ["get-in", ["get", "state"], "door"], "look", null
        ])),
    );
    shop.interface.push("look-at-door".into());

    let replies = send(&mut world, "local:shop", "look-at-door", Value::Null);
    assert!(replies
        .iter()
        .any(|r| r.as_str() == Some("A heavy oak door with an iron lock. Unlocked and closed.")));
}
