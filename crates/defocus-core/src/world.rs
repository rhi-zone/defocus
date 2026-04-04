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
    pub queue: VecDeque<(Identity, Message)>,
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
        self.queue.push_back((to, message));
    }

    pub fn step(&mut self) -> bool {
        let Some((target_id, message)) = self.queue.pop_front() else {
            return false;
        };

        let Some(object) = self.objects.get(&target_id) else {
            return true;
        };

        let Some(handler) = object.handlers.get(&message.verb).cloned() else {
            return true;
        };

        let state_value = Value::Record(object.state.clone());
        let effects = crate::eval::eval_handler(&handler, &message.payload, &state_value);

        for effect in effects {
            match effect {
                Effect::SetState { key, value } => {
                    if let Some(obj) = self.objects.get_mut(&target_id) {
                        obj.state.insert(key, value);
                    }
                }
                Effect::Send { to, message } => {
                    self.queue.push_back((to, message));
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
