use crate::llm::LlmProvider;
use crate::log::{Event, EventLog};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

pub type Identity = String;
pub type Expr = Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub verb: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: Identity,
    pub state: BTreeMap<String, Value>,
    pub handlers: BTreeMap<String, Expr>,
    pub interface: Vec<String>,
    pub children: Vec<Identity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prototype: Option<Identity>,
}

impl Object {
    pub fn new(id: impl Into<Identity>) -> Self {
        Object {
            id: id.into(),
            state: BTreeMap::new(),
            handlers: BTreeMap::new(),
            interface: Vec::new(),
            children: Vec::new(),
            prototype: None,
        }
    }

    pub fn with_state(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.state.insert(key.into(), value.into());
        self
    }

    pub fn with_ref(mut self, key: impl Into<String>, target_id: impl Into<String>) -> Self {
        self.state.insert(
            key.into(),
            Value::Ref {
                id: target_id.into(),
                verbs: None,
            },
        );
        self
    }

    pub fn with_attenuated_ref(
        mut self,
        key: impl Into<String>,
        target_id: impl Into<String>,
        verbs: Vec<String>,
    ) -> Self {
        self.state.insert(
            key.into(),
            Value::Ref {
                id: target_id.into(),
                verbs: Some(verbs),
            },
        );
        self
    }

    pub fn with_handler(mut self, verb: impl Into<String>, handler: Expr) -> Self {
        let verb = verb.into();
        if !self.interface.contains(&verb) {
            self.interface.push(verb.clone());
        }
        self.handlers.insert(verb, handler);
        self
    }

    pub fn with_prototype(mut self, id: impl Into<Identity>) -> Self {
        self.prototype = Some(id.into());
        self
    }

    pub fn stub(id: impl Into<Identity>, verbs: Vec<String>) -> Self {
        Object {
            id: id.into(),
            state: BTreeMap::new(),
            handlers: BTreeMap::new(),
            interface: verbs,
            children: Vec::new(),
            prototype: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Effect {
    SetState { key: String, value: Value },
    Send {
        to: Identity,
        allowed_verbs: Option<Vec<String>>,
        message: Message,
    },
    Schedule {
        at: u64,
        to: Identity,
        message: Message,
    },
    Spawn { object: Object },
    Remove { id: Identity },
    Reply { value: Value },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct World {
    pub objects: BTreeMap<Identity, Object>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub schedule: BTreeMap<u64, Vec<(Identity, Message)>>,
    #[serde(skip)]
    pub queue: VecDeque<(Identity, Message, Option<Identity>)>,
    #[serde(skip)]
    pub log: Option<EventLog>,
    #[serde(skip)]
    pub llm: Option<Arc<dyn LlmProvider>>,
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("objects", &self.objects)
            .field("tick", &self.tick)
            .field("schedule", &self.schedule)
            .field("queue", &self.queue)
            .field("log", &self.log)
            .field("llm", &self.llm.as_ref().map(|_| "<provider>"))
            .finish()
    }
}

impl World {
    pub fn new() -> Self {
        World {
            objects: BTreeMap::new(),
            tick: 0,
            schedule: BTreeMap::new(),
            queue: VecDeque::new(),
            log: None,
            llm: None,
        }
    }

    /// Set the LLM provider for this world. Handlers can use `["llm", prompt-expr]`
    /// to call the provider during evaluation.
    pub fn set_llm(&mut self, provider: impl LlmProvider + 'static) {
        self.llm = Some(Arc::new(provider));
    }

    pub fn add(&mut self, object: Object) {
        self.objects.insert(object.id.clone(), object);
    }

    pub fn send(&mut self, to: Identity, message: Message) {
        self.queue.push_back((to, message, None));
    }

    /// Resolve a handler for a verb by walking the prototype chain.
    /// Returns the handler expression if found. Guards against cycles.
    fn resolve_handler(&self, start_id: &Identity, verb: &str) -> Option<Expr> {
        let mut visited = std::collections::HashSet::new();
        let mut current_id = start_id.clone();
        loop {
            if !visited.insert(current_id.clone()) {
                // Cycle detected
                return None;
            }
            let obj = self.objects.get(&current_id)?;
            if let Some(handler) = obj.handlers.get(verb) {
                return Some(handler.clone());
            }
            match &obj.prototype {
                Some(proto_id) => current_id = proto_id.clone(),
                None => return None,
            }
        }
    }

    /// Process the next queued message. Returns `None` if the queue is empty,
    /// or `Some(replies)` with any Reply values produced by this step.
    pub fn step(&mut self) -> Option<Vec<Value>> {
        let (target_id, message, sender) = self.queue.pop_front()?;

        let Some(object) = self.objects.get(&target_id) else {
            if let Some(ref mut log) = self.log {
                log.events.push(Event {
                    target: target_id,
                    message,
                    sender,
                    replies: Vec::new(),
                });
            }
            return Some(Vec::new());
        };

        // Walk prototype chain to find handler, but use the original object's state
        let state_value = Value::Record(object.state.clone());

        let Some(handler) = self.resolve_handler(&target_id, &message.verb) else {
            if let Some(ref mut log) = self.log {
                log.events.push(Event {
                    target: target_id,
                    message,
                    sender,
                    replies: Vec::new(),
                });
            }
            return Some(Vec::new());
        };

        let effects = crate::eval::eval_handler_with_world(
            &handler,
            &message.payload,
            &state_value,
            &target_id,
            sender.as_deref(),
            self.llm.as_deref(),
            Some(&self.objects),
        );

        let mut replies = Vec::new();
        for effect in effects {
            match effect {
                Effect::SetState { key, value } => {
                    if let Some(obj) = self.objects.get_mut(&target_id) {
                        obj.state.insert(key, value);
                    }
                }
                Effect::Send {
                    to,
                    allowed_verbs,
                    message,
                } => {
                    // Enforce capability attenuation: if the ref had a verb filter,
                    // silently drop messages with disallowed verbs.
                    if let Some(ref verbs) = allowed_verbs {
                        if !verbs.iter().any(|v| v == &message.verb) {
                            continue;
                        }
                    }
                    self.queue.push_back((to, message, Some(target_id.clone())));
                }
                Effect::Schedule { at, to, message } => {
                    self.schedule.entry(at).or_default().push((to, message));
                }
                Effect::Spawn { object } => {
                    self.objects.insert(object.id.clone(), object);
                }
                Effect::Remove { id } => {
                    self.objects.remove(&id);
                }
                Effect::Reply { value } => {
                    replies.push(value);
                }
            }
        }

        if let Some(ref mut log) = self.log {
            log.events.push(Event {
                target: target_id,
                message,
                sender,
                replies: replies.clone(),
            });
        }

        Some(replies)
    }

    /// Process all queued messages until the queue is empty.
    /// Returns all Reply values collected across all steps.
    /// Panics after `limit` iterations to prevent infinite loops.
    pub fn drain(&mut self, limit: usize) -> Vec<Value> {
        let mut all_replies = Vec::new();
        let mut count = 0;
        while let Some(replies) = self.step() {
            all_replies.extend(replies);
            count += 1;
            assert!(count <= limit, "drain exceeded {limit} iterations");
        }
        all_replies
    }

    /// Advance the logical clock to `to_tick`, delivering all scheduled messages
    /// whose tick <= `to_tick`. Processes them through the normal step loop.
    /// Returns all Reply values collected.
    pub fn advance(&mut self, to_tick: u64) -> Vec<Value> {
        assert!(
            to_tick >= self.tick,
            "cannot advance backward: current tick is {}, requested {}",
            self.tick,
            to_tick
        );
        self.tick = to_tick;

        // Collect all scheduled entries up to and including to_tick.
        // We split_off at to_tick+1 to keep everything after to_tick in self.schedule.
        let remaining = self.schedule.split_off(&(to_tick + 1));
        let due = std::mem::replace(&mut self.schedule, remaining);

        for (_tick, messages) in due {
            for (to, message) in messages {
                self.queue.push_back((to, message, None));
            }
        }

        self.drain(10_000)
    }

    /// Advance the logical clock by one tick. Convenience wrapper.
    pub fn advance_one(&mut self) -> Vec<Value> {
        self.advance(self.tick + 1)
    }
}

impl World {
    /// Serialize the world to a JSON value with clean format:
    /// object IDs are map keys, not duplicated inside objects.
    pub fn to_json(&self) -> serde_json::Value {
        let mut objects = serde_json::Map::new();
        for (id, obj) in &self.objects {
            let mut entry = serde_json::Map::new();
            entry.insert(
                "state".to_string(),
                serde_json::to_value(&obj.state).unwrap(),
            );
            entry.insert(
                "handlers".to_string(),
                serde_json::to_value(&obj.handlers).unwrap(),
            );
            entry.insert(
                "interface".to_string(),
                serde_json::to_value(&obj.interface).unwrap(),
            );
            entry.insert(
                "children".to_string(),
                serde_json::to_value(&obj.children).unwrap(),
            );
            if let Some(proto) = &obj.prototype {
                entry.insert(
                    "prototype".to_string(),
                    serde_json::to_value(proto).unwrap(),
                );
            }
            objects.insert(id.clone(), serde_json::Value::Object(entry));
        }

        let mut root = serde_json::json!({
            "version": 1,
            "objects": objects,
        });
        if self.tick > 0 {
            root["tick"] = serde_json::json!(self.tick);
        }
        if !self.schedule.is_empty() {
            root["schedule"] = serde_json::to_value(&self.schedule).unwrap();
        }
        root
    }

    /// Deserialize a world from the JSON format produced by `to_json`.
    pub fn from_json(json: serde_json::Value) -> Result<World, String> {
        let root = json.as_object().ok_or("expected object at root")?;

        let version = root
            .get("version")
            .and_then(|v| v.as_u64())
            .ok_or("missing or invalid version")?;
        if version != 1 {
            return Err(format!("unsupported version: {version}"));
        }

        let objects_val = root
            .get("objects")
            .and_then(|v| v.as_object())
            .ok_or("missing or invalid objects")?;

        let mut world = World::new();
        for (id, obj_json) in objects_val {
            let obj_map = obj_json
                .as_object()
                .ok_or_else(|| format!("expected object for {id}"))?;

            let state: BTreeMap<String, Value> = obj_map
                .get("state")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("invalid state for {id}: {e}"))?
                .unwrap_or_default();

            let handlers: BTreeMap<String, Expr> = obj_map
                .get("handlers")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("invalid handlers for {id}: {e}"))?
                .unwrap_or_default();

            let interface: Vec<String> = obj_map
                .get("interface")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("invalid interface for {id}: {e}"))?
                .unwrap_or_default();

            let children: Vec<Identity> = obj_map
                .get("children")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("invalid children for {id}: {e}"))?
                .unwrap_or_default();

            let prototype: Option<Identity> = obj_map
                .get("prototype")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("invalid prototype for {id}: {e}"))?;

            world.add(Object {
                id: id.clone(),
                state,
                handlers,
                interface,
                children,
                prototype,
            });
        }

        world.tick = root
            .get("tick")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if let Some(sched_val) = root.get("schedule") {
            world.schedule = serde_json::from_value(sched_val.clone())
                .map_err(|e| format!("invalid schedule: {e}"))?;
        }

        Ok(world)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockProvider;

    fn val(j: serde_json::Value) -> Value {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn test_door_frame_with_refs() {
        let mut world = World::new();

        // Door has a ref to frame in state, uses it to send
        let door = Object::new("local:door")
            .with_ref("frame", "local:frame")
            .with_handler(
                "open",
                val(serde_json::json!([
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

        // Frame tracks whether its door is open
        let frame = Object::new("local:frame").with_handler(
            "door-opened",
            val(serde_json::json!(["perform", "set", "doorOpen", true])),
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

        assert_eq!(
            world.objects.get("local:door").unwrap().state.get("open"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            world
                .objects
                .get("local:frame")
                .unwrap()
                .state
                .get("doorOpen"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_string_send_still_works() {
        // Backward compat: string IDs in send still work
        let mut world = World::new();

        let door = Object::new("local:door").with_handler(
            "open",
            val(serde_json::json!([
                "do",
                ["perform", "set", "open", true],
                ["perform", "send", "local:frame", "door-opened", null]
            ])),
        );

        let frame = Object::new("local:frame").with_handler(
            "door-opened",
            val(serde_json::json!(["perform", "set", "doorOpen", true])),
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

        assert_eq!(
            world
                .objects
                .get("local:frame")
                .unwrap()
                .state
                .get("doorOpen"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_self_and_sender_bindings() {
        let mut world = World::new();

        // Object A sends to object B
        let a = Object::new("local:a").with_handler(
            "trigger",
            val(serde_json::json!([
                "perform", "send", "local:b", "ping", null
            ])),
        );

        // Object B records self and sender into state
        let b = Object::new("local:b").with_handler(
            "ping",
            val(serde_json::json!([
                "do",
                ["perform", "set", "got-self", ["get", "self"]],
                ["perform", "set", "got-sender", ["get", "sender"]]
            ])),
        );

        world.add(a);
        world.add(b);
        world.send(
            "local:a".into(),
            Message {
                verb: "trigger".into(),
                payload: Value::Null,
            },
        );
        world.drain(100);

        assert_eq!(
            world.objects.get("local:b").unwrap().state.get("got-self"),
            Some(&Value::Ref {
                id: "local:b".into(),
                verbs: None
            })
        );
        assert_eq!(
            world
                .objects
                .get("local:b")
                .unwrap()
                .state
                .get("got-sender"),
            Some(&Value::Ref {
                id: "local:a".into(),
                verbs: None
            })
        );
    }

    #[test]
    fn test_external_send_has_no_sender() {
        let mut world = World::new();

        let obj = Object::new("local:obj").with_handler(
            "ping",
            val(serde_json::json!([
                "perform",
                "set",
                "got-sender",
                ["get", "sender"]
            ])),
        );

        world.add(obj);
        world.send(
            "local:obj".into(),
            Message {
                verb: "ping".into(),
                payload: Value::Null,
            },
        );
        world.drain(100);

        assert_eq!(
            world
                .objects
                .get("local:obj")
                .unwrap()
                .state
                .get("got-sender"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn test_world_serialization_roundtrip() {
        let mut world = World::new();

        let room = Object::new("local:room")
            .with_state("description", "A dusty room.")
            .with_ref("door", "local:door")
            .with_handler(
                "look",
                val(serde_json::json!([
                    "get-in",
                    ["get", "state"],
                    "description"
                ])),
            );
        let mut room = room;
        room.children.push("local:door".into());

        let door = Object::new("local:door")
            .with_state("open", Value::Bool(false))
            .with_handler(
                "open",
                val(serde_json::json!(["perform", "set", "open", true])),
            );

        world.add(room);
        world.add(door);

        // Add something to the queue to verify it's not serialized
        world.send(
            "local:door".into(),
            Message {
                verb: "open".into(),
                payload: Value::Null,
            },
        );

        let json = world.to_json();

        // Verify structure
        assert_eq!(json["version"], 1);
        assert!(json["objects"]["local:room"]["state"]["door"]["$ref"]
            .as_str()
            .is_some());
        assert_eq!(
            json["objects"]["local:room"]["state"]["door"]["$ref"],
            "local:door"
        );
        assert_eq!(
            json["objects"]["local:room"]["children"],
            serde_json::json!(["local:door"])
        );

        // Round-trip
        let restored = World::from_json(json).unwrap();

        // Queue should be empty after deserialization
        assert!(restored.queue.is_empty());

        // Objects should match
        assert_eq!(restored.objects.len(), 2);

        let restored_room = restored.objects.get("local:room").unwrap();
        assert_eq!(restored_room.id, "local:room");
        assert_eq!(
            restored_room.state.get("description"),
            Some(&Value::String("A dusty room.".into()))
        );
        assert_eq!(
            restored_room.state.get("door"),
            Some(&Value::Ref {
                id: "local:door".into(),
                verbs: None
            })
        );
        assert_eq!(restored_room.interface, vec!["look"]);
        assert_eq!(restored_room.children, vec!["local:door"]);

        let restored_door = restored.objects.get("local:door").unwrap();
        assert_eq!(restored_door.state.get("open"), Some(&Value::Bool(false)));
        assert_eq!(restored_door.interface, vec!["open"]);

        // Handlers survived
        assert!(restored_room.handlers.contains_key("look"));
        assert!(restored_door.handlers.contains_key("open"));
    }

    #[test]
    fn test_ref_serde_roundtrip() {
        let v = Value::Ref {
            id: "local:frame".into(),
            verbs: None,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"$ref":"local:frame"}"#);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, v);
    }

    #[test]
    fn test_llm_call_returns_response_and_replies() {
        let mut world = World::new();
        world.set_llm(
            MockProvider::new("I don't understand.")
                .with_response("hello", "Greetings, traveler!"),
        );

        // NPC uses LLM to generate a response and replies with it
        let npc = Object::new("local:npc").with_handler(
            "talk",
            val(serde_json::json!([
                "let", "response",
                ["llm", ["concat", "The player says: ", ["get", "payload"]]],
                ["do",
                    ["perform", "set", "last-response", ["get", "response"]],
                    ["perform", "reply", ["get", "response"]]
                ]
            ])),
        );
        world.add(npc);
        world.send(
            "local:npc".into(),
            Message {
                verb: "talk".into(),
                payload: Value::String("hello".into()),
            },
        );

        let replies = world.drain(100);
        assert_eq!(replies, vec![Value::String("Greetings, traveler!".into())]);
        assert_eq!(
            world
                .objects
                .get("local:npc")
                .unwrap()
                .state
                .get("last-response"),
            Some(&Value::String("Greetings, traveler!".into()))
        );
    }

    #[test]
    fn test_llm_no_provider_returns_null() {
        let mut world = World::new();
        // No LLM provider set — ["llm", ...] returns null

        let npc = Object::new("local:npc").with_handler(
            "talk",
            val(serde_json::json!([
                "let", "response", ["llm", "anything"],
                ["if", ["get", "response"],
                    ["perform", "reply", ["get", "response"]],
                    ["perform", "reply", "I have nothing to say."]
                ]
            ])),
        );
        world.add(npc);
        world.send(
            "local:npc".into(),
            Message {
                verb: "talk".into(),
                payload: Value::Null,
            },
        );

        let replies = world.drain(100);
        assert_eq!(
            replies,
            vec![Value::String("I have nothing to say.".into())]
        );
    }

    #[test]
    fn test_llm_driven_npc_stores_state() {
        let mut world = World::new();
        world.set_llm(
            MockProvider::new("*silence*")
                .with_response("greet", "Welcome to my shop!")
                .with_response("buy", "That'll be 10 gold."),
        );

        // NPC talk handler uses LLM for dynamic response, stores in state
        let npc = Object::new("local:shopkeeper")
            .with_state("interaction-count", Value::Int(0))
            .with_handler(
                "talk",
                val(serde_json::json!([
                    "let", "response",
                    ["llm", ["concat", "Action: ", ["get", "payload"]]],
                    ["do",
                        ["perform", "set", "last-response", ["get", "response"]],
                        ["perform", "set", "interaction-count",
                            ["+", ["get-in", ["get", "state"], "interaction-count"], 1]],
                        ["perform", "reply", ["get", "response"]]
                    ]
                ])),
            );
        world.add(npc);

        // First interaction
        world.send(
            "local:shopkeeper".into(),
            Message {
                verb: "talk".into(),
                payload: Value::String("greet".into()),
            },
        );
        let replies = world.drain(100);
        assert_eq!(
            replies,
            vec![Value::String("Welcome to my shop!".into())]
        );

        // Second interaction
        world.send(
            "local:shopkeeper".into(),
            Message {
                verb: "talk".into(),
                payload: Value::String("buy sword".into()),
            },
        );
        let replies = world.drain(100);
        assert_eq!(
            replies,
            vec![Value::String("That'll be 10 gold.".into())]
        );

        let shopkeeper = world.objects.get("local:shopkeeper").unwrap();
        assert_eq!(
            shopkeeper.state.get("interaction-count"),
            Some(&Value::Int(2))
        );
        assert_eq!(
            shopkeeper.state.get("last-response"),
            Some(&Value::String("That'll be 10 gold.".into()))
        );
    }

    #[test]
    fn test_query_from_handler() {
        let mut world = World::new();

        // Two hostile NPCs and one friendly
        let hostile1 = Object::new("npc:orc")
            .with_state("mood", "hostile")
            .with_handler(
                "alert",
                val(serde_json::json!(["perform", "set", "alerted", true])),
            );
        let hostile2 = Object::new("npc:goblin")
            .with_state("mood", "hostile")
            .with_handler(
                "alert",
                val(serde_json::json!(["perform", "set", "alerted", true])),
            );
        let friendly = Object::new("npc:merchant")
            .with_state("mood", "friendly")
            .with_handler(
                "alert",
                val(serde_json::json!(["perform", "set", "alerted", true])),
            );

        // Player handler: on "shout", query for hostile NPCs, send "alert" to each
        let player = Object::new("player").with_handler(
            "shout",
            val(serde_json::json!([
                "let", "hostiles",
                ["query", {"state": {"mood": "hostile"}, "interface": ["array", "alert"]}],
                ["do",
                    ["perform", "set", "found-count",
                        ["length", ["get", "hostiles"]]],
                    ["map", ["get", "hostiles"],
                        ["fn", ["target"],
                            ["perform", "send", ["get", "target"], "alert", null]]]]
            ])),
        );

        world.add(hostile1);
        world.add(hostile2);
        world.add(friendly);
        world.add(player);

        world.send(
            "player".into(),
            Message {
                verb: "shout".into(),
                payload: Value::Null,
            },
        );
        world.drain(100);

        // Player found 2 hostile NPCs
        assert_eq!(
            world.objects.get("player").unwrap().state.get("found-count"),
            Some(&Value::Int(2))
        );
        // Both hostile NPCs got alerted
        assert_eq!(
            world.objects.get("npc:orc").unwrap().state.get("alerted"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            world.objects.get("npc:goblin").unwrap().state.get("alerted"),
            Some(&Value::Bool(true))
        );
        // Friendly NPC was not alerted
        assert_eq!(
            world
                .objects
                .get("npc:merchant")
                .unwrap()
                .state
                .get("alerted"),
            None
        );
    }
}
