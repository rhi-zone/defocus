//! Interconnect Authority adapter for defocus.
//!
//! Exposes a defocus [`World`] as an Interconnect room, allowing multiplayer
//! interaction through the Authority protocol.
//!
//! # Dependency note
//!
//! This crate is designed to implement `interconnect_core::Authority` (via the
//! `SimpleAuthority` blanket impl). The `interconnect-core` dependency is
//! currently commented out in `Cargo.toml` because it is not yet published on
//! crates.io, and the ecosystem prohibits path dependencies. The adapter types
//! and logic are fully implemented; only the trait impl block is stubbed as a
//! comment.

use defocus_core::value::Value;
use defocus_core::world::{Message, World};
use std::collections::HashMap;

/// A player's message to the world.
#[derive(Debug, Clone)]
pub struct WorldIntent {
    /// Object ID the player is addressing.
    pub target: String,
    /// The message to deliver.
    pub message: Message,
}

/// The world state broadcast to clients.
#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    /// `World::to_json()` output.
    pub world: serde_json::Value,
    /// Replies from the last step.
    pub replies: Vec<Value>,
}

/// Player state that travels between rooms.
#[derive(Debug, Clone)]
pub struct PlayerPassport {
    /// Unique player identifier.
    pub player_id: String,
    /// Arbitrary player state carried across rooms.
    pub state: std::collections::BTreeMap<String, Value>,
}

/// Error type for authority operations.
#[derive(Debug)]
pub enum AuthorityError {
    /// The player is not registered.
    UnknownPlayer(String),
    /// The target object does not exist.
    UnknownTarget(String),
    /// The player's avatar does not hold a ref to the target.
    NoAccess {
        player_id: String,
        target: String,
    },
    /// A verb was blocked by capability attenuation.
    VerbDenied {
        target: String,
        verb: String,
    },
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorityError::UnknownPlayer(id) => write!(f, "unknown player: {id}"),
            AuthorityError::UnknownTarget(id) => write!(f, "unknown target object: {id}"),
            AuthorityError::NoAccess { player_id, target } => {
                write!(f, "player {player_id} has no ref to {target}")
            }
            AuthorityError::VerbDenied { target, verb } => {
                write!(f, "verb {verb:?} denied on {target}")
            }
        }
    }
}

impl std::error::Error for AuthorityError {}

/// Adapter that bridges a defocus [`World`] to the Interconnect authority protocol.
///
/// Each player is associated with an "avatar" object in the world. When a player
/// sends an intent, the authority checks that the player's avatar holds a ref
/// (direct or via state) to the target object, then delivers the message.
pub struct DefocusAuthority {
    world: World,
    /// player_id -> avatar object_id
    players: HashMap<String, String>,
}

impl DefocusAuthority {
    /// Create a new authority wrapping the given world.
    pub fn new(world: World) -> Self {
        DefocusAuthority {
            world,
            players: HashMap::new(),
        }
    }

    /// Register a player's avatar object in the world.
    pub fn add_player(&mut self, player_id: &str, object_id: &str) {
        self.players
            .insert(player_id.to_string(), object_id.to_string());
    }

    /// Remove a player.
    pub fn remove_player(&mut self, player_id: &str) {
        self.players.remove(player_id);
    }

    /// Get a reference to the underlying world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Get a mutable reference to the underlying world.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Generate a snapshot of the current world state.
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            world: self.world.to_json(),
            replies: Vec::new(),
        }
    }

    /// Handle an intent from a player.
    ///
    /// Checks that:
    /// 1. The player is registered with an avatar object.
    /// 2. The target object exists.
    /// 3. The player's avatar holds a ref to the target (or the target *is* the avatar).
    /// 4. If the ref is attenuated, the verb is allowed.
    ///
    /// Then sends the message and drains the world queue, returning a snapshot
    /// with the collected replies.
    pub fn handle_intent(
        &mut self,
        player_id: &str,
        intent: WorldIntent,
    ) -> Result<WorldSnapshot, AuthorityError> {
        let avatar_id = self
            .players
            .get(player_id)
            .ok_or_else(|| AuthorityError::UnknownPlayer(player_id.to_string()))?
            .clone();

        // Players can always message their own avatar directly.
        if intent.target == avatar_id {
            self.world.send(
                intent.target,
                intent.message,
            );
            let replies = self.world.drain(10_000);
            return Ok(WorldSnapshot {
                world: self.world.to_json(),
                replies,
            });
        }

        // Target must exist.
        if !self.world.objects.contains_key(&intent.target) {
            return Err(AuthorityError::UnknownTarget(intent.target));
        }

        // Check that the avatar holds a ref to the target.
        let avatar = self
            .world
            .objects
            .get(&avatar_id)
            .ok_or_else(|| AuthorityError::UnknownPlayer(player_id.to_string()))?;

        let allowed_verbs = find_ref_to(&avatar.state, &intent.target);
        match allowed_verbs {
            None => {
                return Err(AuthorityError::NoAccess {
                    player_id: player_id.to_string(),
                    target: intent.target,
                });
            }
            Some(Some(verbs)) => {
                if !verbs.iter().any(|v| v == &intent.message.verb) {
                    return Err(AuthorityError::VerbDenied {
                        target: intent.target,
                        verb: intent.message.verb,
                    });
                }
            }
            Some(None) => {
                // Unrestricted ref — all verbs allowed.
            }
        }

        self.world.send(intent.target, intent.message);
        let replies = self.world.drain(10_000);
        Ok(WorldSnapshot {
            world: self.world.to_json(),
            replies,
        })
    }
}

/// Search an object's state for a `Value::Ref` pointing to `target_id`.
///
/// Returns:
/// - `None` if no ref to the target exists.
/// - `Some(None)` if an unrestricted ref exists.
/// - `Some(Some(verbs))` if an attenuated ref exists.
fn find_ref_to(
    state: &std::collections::BTreeMap<String, Value>,
    target_id: &str,
) -> Option<Option<Vec<String>>> {
    for value in state.values() {
        if let Value::Ref { id, verbs } = value {
            if id == target_id {
                return Some(verbs.clone());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Interconnect Authority trait impl (stubbed)
// ---------------------------------------------------------------------------
//
// When interconnect-core is available as a crate dependency, uncomment the
// dependency in Cargo.toml and replace this comment block with:
//
// impl interconnect_core::authority::SimpleAuthority for DefocusAuthority {
//     type Intent = WorldIntent;
//     type Snapshot = WorldSnapshot;
//     type Passport = PlayerPassport;
//     type Error = AuthorityError;
//
//     fn on_connect(&mut self, session: &interconnect_core::authority::Session) -> Result<(), Self::Error> {
//         // Create or look up an avatar object for this session.
//         // For now, the caller must pre-register via add_player().
//         Ok(())
//     }
//
//     fn on_transfer_in(
//         &mut self,
//         session: &interconnect_core::authority::Session,
//         passport: PlayerPassport,
//     ) -> Result<interconnect_core::authority::ImportResult<PlayerPassport>, Self::Error> {
//         // Apply import policy: accept the passport state, create avatar.
//         self.add_player(&passport.player_id, &format!("player:{}", passport.player_id));
//         let mut avatar = Object::new(format!("player:{}", passport.player_id));
//         for (k, v) in &passport.state {
//             avatar = avatar.with_state(k.clone(), v.clone());
//         }
//         self.world.add(avatar);
//         Ok(interconnect_core::authority::ImportResult::accept(passport))
//     }
//
//     fn on_disconnect(&mut self, session: &interconnect_core::authority::Session) {
//         self.remove_player(&session.identity);
//     }
//
//     fn handle_intent(
//         &mut self,
//         session: &interconnect_core::authority::Session,
//         intent: WorldIntent,
//     ) -> Result<(), Self::Error> {
//         self.handle_intent(&session.identity, intent)?;
//         Ok(())
//     }
//
//     fn snapshot(&self) -> WorldSnapshot {
//         self.snapshot()
//     }
//
//     fn emit_passport(&self, session: &interconnect_core::authority::Session) -> PlayerPassport {
//         let player_id = session.identity.clone();
//         let state = self.players.get(&player_id)
//             .and_then(|obj_id| self.world.objects.get(obj_id))
//             .map(|obj| obj.state.clone())
//             .unwrap_or_default();
//         PlayerPassport { player_id, state }
//     }
//
//     fn validate_destination(&self, _destination: &str) -> bool {
//         // Accept any destination for now.
//         true
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use defocus_core::value::Value;
    use defocus_core::world::{Message, Object, World};

    fn val(j: serde_json::Value) -> Value {
        serde_json::from_value(j).unwrap()
    }

    /// Helper: build a small world with a room and an NPC.
    /// The player avatar has a ref to the NPC.
    fn test_world() -> (World, String, String, String) {
        let mut world = World::new();

        let room_id = "local:room".to_string();
        let npc_id = "local:npc".to_string();
        let player_id = "local:player-avatar".to_string();

        let room = Object::new(&room_id)
            .with_state("description", "A quiet room.");

        let npc = Object::new(&npc_id)
            .with_state("mood", "neutral")
            .with_handler(
                "greet",
                val(serde_json::json!([
                    "do",
                    ["perform", "set", "mood", "happy"],
                    ["perform", "reply", "Hello, traveler!"]
                ])),
            )
            .with_handler(
                "ask",
                val(serde_json::json!([
                    "perform", "reply",
                    ["concat", "You asked about: ", ["get", "payload"]]
                ])),
            );

        let player_avatar = Object::new(&player_id)
            .with_ref("npc", &npc_id)
            .with_ref("room", &room_id)
            .with_handler(
                "ping",
                val(serde_json::json!(["perform", "reply", "pong"])),
            );

        world.add(room);
        world.add(npc);
        world.add(player_avatar);

        (world, room_id, npc_id, player_id)
    }

    #[test]
    fn test_single_player_intent() {
        let (world, _room_id, npc_id, avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &avatar_id);

        let snapshot = authority
            .handle_intent(
                "alice",
                WorldIntent {
                    target: npc_id.clone(),
                    message: Message {
                        verb: "greet".into(),
                        payload: Value::Null,
                    },
                },
            )
            .unwrap();

        // NPC should have replied.
        assert_eq!(
            snapshot.replies,
            vec![Value::String("Hello, traveler!".into())]
        );

        // NPC mood should have changed in the snapshot.
        let npc_state = &snapshot.world["objects"]["local:npc"]["state"];
        assert_eq!(npc_state["mood"], serde_json::json!("happy"));
    }

    #[test]
    fn test_player_messages_own_avatar() {
        let (world, _room_id, _npc_id, avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &avatar_id);

        // Players can always message their own avatar.
        let snapshot = authority
            .handle_intent(
                "alice",
                WorldIntent {
                    target: avatar_id.clone(),
                    message: Message {
                        verb: "ping".into(),
                        payload: Value::Null,
                    },
                },
            )
            .unwrap();

        assert_eq!(snapshot.replies, vec![Value::String("pong".into())]);
    }

    #[test]
    fn test_unknown_player_error() {
        let (world, _room_id, npc_id, _avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        // Don't register any player.

        let result = authority.handle_intent(
            "nobody",
            WorldIntent {
                target: npc_id,
                message: Message {
                    verb: "greet".into(),
                    payload: Value::Null,
                },
            },
        );

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), AuthorityError::UnknownPlayer(id) if id == "nobody")
        );
    }

    #[test]
    fn test_no_access_error() {
        let (world, _room_id, _npc_id, avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &avatar_id);

        // Try to message an object the avatar doesn't have a ref to.
        // Add a hidden object with no ref from the avatar.
        authority
            .world_mut()
            .add(Object::new("local:hidden").with_handler(
                "secret",
                val(serde_json::json!(["perform", "reply", "you shouldn't see this"])),
            ));

        let result = authority.handle_intent(
            "alice",
            WorldIntent {
                target: "local:hidden".into(),
                message: Message {
                    verb: "secret".into(),
                    payload: Value::Null,
                },
            },
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AuthorityError::NoAccess { .. }
        ));
    }

    #[test]
    fn test_unknown_target_error() {
        let (world, _room_id, _npc_id, avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &avatar_id);

        let result = authority.handle_intent(
            "alice",
            WorldIntent {
                target: "local:nonexistent".into(),
                message: Message {
                    verb: "look".into(),
                    payload: Value::Null,
                },
            },
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AuthorityError::UnknownTarget(..)
        ));
    }

    #[test]
    fn test_multiple_players() {
        let (mut world, _room_id, npc_id, alice_avatar_id) = test_world();

        // Add a second player avatar with a ref to the NPC.
        let bob_avatar_id = "local:bob-avatar".to_string();
        let bob_avatar = Object::new(&bob_avatar_id).with_ref("npc", &npc_id);
        world.add(bob_avatar);

        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &alice_avatar_id);
        authority.add_player("bob", &bob_avatar_id);

        // Alice greets the NPC.
        let snap1 = authority
            .handle_intent(
                "alice",
                WorldIntent {
                    target: npc_id.clone(),
                    message: Message {
                        verb: "greet".into(),
                        payload: Value::Null,
                    },
                },
            )
            .unwrap();
        assert_eq!(
            snap1.replies,
            vec![Value::String("Hello, traveler!".into())]
        );

        // Bob asks the NPC something — NPC mood should already be "happy" from Alice's greet.
        let snap2 = authority
            .handle_intent(
                "bob",
                WorldIntent {
                    target: npc_id.clone(),
                    message: Message {
                        verb: "ask".into(),
                        payload: Value::String("directions".into()),
                    },
                },
            )
            .unwrap();
        assert_eq!(
            snap2.replies,
            vec![Value::String("You asked about: directions".into())]
        );

        // Verify shared world state: NPC mood is still happy.
        assert_eq!(
            snap2.world["objects"]["local:npc"]["state"]["mood"],
            serde_json::json!("happy")
        );
    }

    #[test]
    fn test_remove_player() {
        let (world, _room_id, npc_id, avatar_id) = test_world();
        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", &avatar_id);

        // Works before removal.
        assert!(authority
            .handle_intent(
                "alice",
                WorldIntent {
                    target: npc_id.clone(),
                    message: Message {
                        verb: "greet".into(),
                        payload: Value::Null,
                    },
                },
            )
            .is_ok());

        // Remove and verify failure.
        authority.remove_player("alice");

        let result = authority.handle_intent(
            "alice",
            WorldIntent {
                target: npc_id,
                message: Message {
                    verb: "greet".into(),
                    payload: Value::Null,
                },
            },
        );
        assert!(matches!(
            result.unwrap_err(),
            AuthorityError::UnknownPlayer(..)
        ));
    }

    #[test]
    fn test_attenuated_ref_blocks_verb() {
        let mut world = World::new();

        let npc = Object::new("local:npc")
            .with_handler(
                "look",
                val(serde_json::json!(["perform", "reply", "An old merchant."])),
            )
            .with_handler(
                "steal",
                val(serde_json::json!(["perform", "reply", "You stole something!"])),
            );

        // Avatar has an attenuated ref: can only "look", not "steal".
        let avatar = Object::new("local:avatar").with_attenuated_ref(
            "npc",
            "local:npc",
            vec!["look".into()],
        );

        world.add(npc);
        world.add(avatar);

        let mut authority = DefocusAuthority::new(world);
        authority.add_player("alice", "local:avatar");

        // "look" should work.
        let snap = authority
            .handle_intent(
                "alice",
                WorldIntent {
                    target: "local:npc".into(),
                    message: Message {
                        verb: "look".into(),
                        payload: Value::Null,
                    },
                },
            )
            .unwrap();
        assert_eq!(
            snap.replies,
            vec![Value::String("An old merchant.".into())]
        );

        // "steal" should be denied.
        let result = authority.handle_intent(
            "alice",
            WorldIntent {
                target: "local:npc".into(),
                message: Message {
                    verb: "steal".into(),
                    payload: Value::Null,
                },
            },
        );
        assert!(matches!(
            result.unwrap_err(),
            AuthorityError::VerbDenied { .. }
        ));
    }

    #[test]
    fn test_snapshot_without_intent() {
        let (world, _room_id, _npc_id, _avatar_id) = test_world();
        let authority = DefocusAuthority::new(world);

        let snapshot = authority.snapshot();
        // Should contain the world objects.
        assert!(snapshot.world["objects"]["local:room"].is_object());
        assert!(snapshot.world["objects"]["local:npc"].is_object());
        // No replies since nothing was processed.
        assert!(snapshot.replies.is_empty());
    }
}
