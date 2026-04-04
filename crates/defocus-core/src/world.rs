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

    pub fn step(&mut self) -> bool {
        let Some((target_id, message, sender)) = self.queue.pop_front() else {
            return false;
        };

        let Some(object) = self.objects.get(&target_id) else {
            return true;
        };

        let Some(handler) = object.handlers.get(&message.verb).cloned() else {
            return true;
        };

        let state_value = Value::Record(object.state.clone());
        let effects = crate::eval::eval_handler(
            &handler,
            &message.payload,
            &state_value,
            &target_id,
            sender.as_deref(),
        );

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
            }
        }

        true
    }

    /// Process all queued messages until the queue is empty.
    /// Returns the number of messages processed.
    /// Panics after `limit` iterations to prevent infinite loops.
    pub fn drain(&mut self, limit: usize) -> usize {
        let mut count = 0;
        while self.step() {
            count += 1;
            assert!(count <= limit, "drain exceeded {limit} iterations");
        }
        count
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
    fn test_ref_serde_roundtrip() {
        let v = Value::Ref("local:frame".into());
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"$ref":"local:frame"}"#);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, v);
    }
}
