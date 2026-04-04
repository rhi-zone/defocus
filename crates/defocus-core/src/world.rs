use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

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
}

impl Object {
    pub fn new(id: impl Into<Identity>) -> Self {
        Object {
            id: id.into(),
            state: BTreeMap::new(),
            handlers: BTreeMap::new(),
            interface: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_state(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.state.insert(key.into(), value.into());
        self
    }

    pub fn with_ref(mut self, key: impl Into<String>, target_id: impl Into<String>) -> Self {
        self.state.insert(key.into(), Value::Ref(target_id.into()));
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

    pub fn stub(id: impl Into<Identity>, verbs: Vec<String>) -> Self {
        Object {
            id: id.into(),
            state: BTreeMap::new(),
            handlers: BTreeMap::new(),
            interface: verbs,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Effect {
    SetState { key: String, value: Value },
    Send { to: Identity, message: Message },
    Spawn { object: Object },
    Remove { id: Identity },
    Reply { value: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub objects: BTreeMap<Identity, Object>,
    #[serde(skip)]
    pub queue: VecDeque<(Identity, Message, Option<Identity>)>,
}

impl World {
    pub fn new() -> Self {
        World {
            objects: BTreeMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn add(&mut self, object: Object) {
        self.objects.insert(object.id.clone(), object);
    }

    pub fn send(&mut self, to: Identity, message: Message) {
        self.queue.push_back((to, message, None));
    }

    /// Process the next queued message. Returns `None` if the queue is empty,
    /// or `Some(replies)` with any Reply values produced by this step.
    pub fn step(&mut self) -> Option<Vec<Value>> {
        let (target_id, message, sender) = self.queue.pop_front()?;

        let Some(object) = self.objects.get(&target_id) else {
            return Some(Vec::new());
        };

        let Some(handler) = object.handlers.get(&message.verb).cloned() else {
            return Some(Vec::new());
        };

        let state_value = Value::Record(object.state.clone());
        let effects = crate::eval::eval_handler(
            &handler,
            &message.payload,
            &state_value,
            &target_id,
            sender.as_deref(),
        );

        let mut replies = Vec::new();
        for effect in effects {
            match effect {
                Effect::SetState { key, value } => {
                    if let Some(obj) = self.objects.get_mut(&target_id) {
                        obj.state.insert(key, value);
                    }
                }
                Effect::Send { to, message } => {
                    self.queue.push_back((to, message, Some(target_id.clone())));
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
            objects.insert(id.clone(), serde_json::Value::Object(entry));
        }

        serde_json::json!({
            "version": 1,
            "objects": objects,
        })
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

            world.add(Object {
                id: id.clone(),
                state,
                handlers,
                interface,
                children,
            });
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
            Some(&Value::Ref("local:b".into()))
        );
        assert_eq!(
            world
                .objects
                .get("local:b")
                .unwrap()
                .state
                .get("got-sender"),
            Some(&Value::Ref("local:a".into()))
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
            Some(&Value::Ref("local:door".into()))
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
        let v = Value::Ref("local:frame".into());
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"$ref":"local:frame"}"#);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, v);
    }
}
