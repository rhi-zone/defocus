use crate::value::Value;
use crate::world::{Identity, Message, World};
use serde::{Deserialize, Serialize};

/// A single dispatched message and its results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub target: Identity,
    pub message: Message,
    pub sender: Option<Identity>,
    pub replies: Vec<Value>,
}

/// A sequence of events, serializable for persistence alongside world snapshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventLog {
    pub events: Vec<Event>,
}

impl EventLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a new log containing events `0..index` (the prefix up to the branch point).
    pub fn branch_at(&self, index: usize) -> EventLog {
        EventLog {
            events: self.events[..index.min(self.events.len())].to_vec(),
        }
    }

    /// Clone the world, replay the log on it, return the new world and all replies.
    pub fn replay_from(world: &World, log: &EventLog) -> (World, Vec<Value>) {
        let mut w = world.clone();
        let replies = w.replay(log);
        (w, replies)
    }
}

impl World {
    /// Enable event logging. Future `step()` calls will record events.
    pub fn enable_logging(&mut self) {
        if self.log.is_none() {
            self.log = Some(EventLog::new());
        }
    }

    /// Disable event logging.
    pub fn disable_logging(&mut self) {
        self.log = None;
    }

    /// Take the current log, leaving `None` in its place.
    pub fn take_log(&mut self) -> Option<EventLog> {
        self.log.take()
    }

    /// Re-dispatch all messages from a log in order, collecting all replies.
    pub fn replay(&mut self, log: &EventLog) -> Vec<Value> {
        let mut all_replies = Vec::new();
        for event in &log.events {
            self.queue.push_back((
                event.target.clone(),
                event.message.clone(),
                event.sender.clone(),
            ));
            // Drain only the messages spawned by this event before moving to next
            while let Some(replies) = self.step() {
                all_replies.extend(replies);
            }
        }
        all_replies
    }

    /// Given the original world (pre-log) and a log, replay up to `index`,
    /// return the new world state and the truncated log.
    pub fn fork_at(&self, log: &EventLog, index: usize) -> (World, EventLog) {
        let truncated = log.branch_at(index);
        let (world, _) = EventLog::replay_from(self, &truncated);
        (world, truncated)
    }
}
