use crate::log::{Event, EventLog};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl World {
    pub fn new() -> Self {
        World {
            objects: BTreeMap::new(),
            tick: 0,
            schedule: BTreeMap::new(),
            queue: VecDeque::new(),
            log: None,
        }
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
}
