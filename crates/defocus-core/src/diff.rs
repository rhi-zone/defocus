use crate::value::Value;
use crate::world::{Identity, Object, World};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Represents the difference between two world states.
/// Computed by `World::diff` and applied by `World::apply_diff`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WorldDiff {
    /// Objects that were added (full object data).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub added: BTreeMap<Identity, Object>,
    /// Objects that were removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<Identity>,
    /// Objects whose state changed (only changed keys).
    /// `None` value means the key was removed; `Some(v)` means it was set to `v`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub state_changes: BTreeMap<Identity, BTreeMap<String, Option<Value>>>,
    /// Objects whose handlers changed.
    /// `None` value means the handler was removed; `Some(v)` means it was set to `v`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub handler_changes: BTreeMap<Identity, BTreeMap<String, Option<Value>>>,
    /// Tick change (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick: Option<u64>,
}

impl WorldDiff {
    /// Returns true if this diff represents no changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.state_changes.is_empty()
            && self.handler_changes.is_empty()
            && self.tick.is_none()
    }
}

/// Compute the key-level diff between two BTreeMaps.
/// Returns only keys that differ: `Some(v)` for added/changed, `None` for removed.
fn diff_maps(
    old: &BTreeMap<String, Value>,
    new: &BTreeMap<String, Value>,
) -> BTreeMap<String, Option<Value>> {
    let mut changes = BTreeMap::new();

    // Keys in new that are different or absent in old
    for (key, new_val) in new {
        match old.get(key) {
            Some(old_val) if old_val == new_val => {}
            _ => {
                changes.insert(key.clone(), Some(new_val.clone()));
            }
        }
    }

    // Keys in old that are absent in new (removals)
    for key in old.keys() {
        if !new.contains_key(key) {
            changes.insert(key.clone(), None);
        }
    }

    changes
}

impl World {
    /// Compute the diff from `self` to `other`.
    /// "What changed to get from self to other?"
    pub fn diff(&self, other: &World) -> WorldDiff {
        let mut diff = WorldDiff::default();

        // Tick change
        if self.tick != other.tick {
            diff.tick = Some(other.tick);
        }

        // Objects in other but not in self → added
        for (id, obj) in &other.objects {
            if !self.objects.contains_key(id) {
                diff.added.insert(id.clone(), obj.clone());
            }
        }

        // Objects in self but not in other → removed
        for id in self.objects.keys() {
            if !other.objects.contains_key(id) {
                diff.removed.push(id.clone());
            }
        }

        // Objects in both → check for state and handler changes
        for (id, old_obj) in &self.objects {
            if let Some(new_obj) = other.objects.get(id) {
                let state_diff = diff_maps(&old_obj.state, &new_obj.state);
                if !state_diff.is_empty() {
                    diff.state_changes.insert(id.clone(), state_diff);
                }

                let handler_diff = diff_maps(&old_obj.handlers, &new_obj.handlers);
                if !handler_diff.is_empty() {
                    diff.handler_changes.insert(id.clone(), handler_diff);
                }
            }
        }

        diff
    }

    /// Apply a diff to this world, producing the new state in-place.
    pub fn apply_diff(&mut self, diff: &WorldDiff) {
        // Apply tick change
        if let Some(tick) = diff.tick {
            self.tick = tick;
        }

        // Remove objects
        for id in &diff.removed {
            self.objects.remove(id);
        }

        // Add objects
        for (id, obj) in &diff.added {
            self.objects.insert(id.clone(), obj.clone());
        }

        // Apply state changes
        for (id, changes) in &diff.state_changes {
            if let Some(obj) = self.objects.get_mut(id) {
                for (key, value) in changes {
                    match value {
                        Some(v) => {
                            obj.state.insert(key.clone(), v.clone());
                        }
                        None => {
                            obj.state.remove(key);
                        }
                    }
                }
            }
        }

        // Apply handler changes
        for (id, changes) in &diff.handler_changes {
            if let Some(obj) = self.objects.get_mut(id) {
                for (key, value) in changes {
                    match value {
                        Some(v) => {
                            obj.handlers.insert(key.clone(), v.clone());
                            // Keep interface in sync
                            if !obj.interface.contains(key) {
                                obj.interface.push(key.clone());
                            }
                        }
                        None => {
                            obj.handlers.remove(key);
                            obj.interface.retain(|v| v != key);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(j: serde_json::Value) -> Value {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn test_no_changes() {
        let mut w1 = World::new();
        w1.add(Object::new("obj:a").with_state("x", 1i64));
        let w2 = w1.clone();

        let diff = w1.diff(&w2);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_state_change() {
        let mut w1 = World::new();
        w1.add(Object::new("obj:a").with_state("x", 1i64).with_state("y", 2i64));

        let mut w2 = w1.clone();
        w2.objects
            .get_mut("obj:a")
            .unwrap()
            .state
            .insert("x".into(), Value::Int(42));

        let diff = w1.diff(&w2);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.handler_changes.is_empty());
        assert_eq!(diff.state_changes.len(), 1);
        let obj_changes = diff.state_changes.get("obj:a").unwrap();
        assert_eq!(obj_changes.len(), 1);
        assert_eq!(obj_changes.get("x"), Some(&Some(Value::Int(42))));
    }

    #[test]
    fn test_object_added() {
        let mut w1 = World::new();
        w1.add(Object::new("obj:a"));

        let mut w2 = w1.clone();
        w2.add(Object::new("obj:b").with_state("name", "new"));

        let diff = w1.diff(&w2);
        assert_eq!(diff.added.len(), 1);
        assert!(diff.added.contains_key("obj:b"));
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_object_removed() {
        let mut w1 = World::new();
        w1.add(Object::new("obj:a"));
        w1.add(Object::new("obj:b"));

        let mut w2 = w1.clone();
        w2.objects.remove("obj:b");

        let diff = w1.diff(&w2);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["obj:b".to_string()]);
    }

    #[test]
    fn test_handler_change() {
        let mut w1 = World::new();
        w1.add(
            Object::new("obj:a").with_handler(
                "look",
                val(serde_json::json!(["perform", "reply", "hello"])),
            ),
        );

        let mut w2 = w1.clone();
        let new_handler = val(serde_json::json!(["perform", "reply", "goodbye"]));
        w2.objects
            .get_mut("obj:a")
            .unwrap()
            .handlers
            .insert("look".into(), new_handler.clone());

        let diff = w1.diff(&w2);
        assert!(diff.state_changes.is_empty());
        assert_eq!(diff.handler_changes.len(), 1);
        let handler_changes = diff.handler_changes.get("obj:a").unwrap();
        assert_eq!(handler_changes.get("look"), Some(&Some(new_handler)));
    }

    #[test]
    fn test_apply_diff_roundtrip() {
        let mut w1 = World::new();
        w1.add(
            Object::new("obj:a")
                .with_state("x", 1i64)
                .with_state("y", 2i64),
        );
        w1.add(Object::new("obj:b").with_state("name", "bob"));

        let mut w2 = World::new();
        w2.tick = 5;
        w2.add(
            Object::new("obj:a")
                .with_state("x", 42i64)
                .with_state("z", "new"),
        );
        w2.add(Object::new("obj:c").with_state("name", "charlie"));

        let diff = w1.diff(&w2);

        let mut result = w1.clone();
        result.apply_diff(&diff);

        // Verify result matches w2 for objects and tick
        assert_eq!(result.tick, w2.tick);
        assert_eq!(result.objects.len(), w2.objects.len());

        for (id, expected_obj) in &w2.objects {
            let actual_obj = result.objects.get(id).expect("missing object");
            assert_eq!(actual_obj.state, expected_obj.state);
            assert_eq!(actual_obj.handlers, expected_obj.handlers);
        }
    }

    #[test]
    fn test_diff_serialization() {
        let mut w1 = World::new();
        w1.add(Object::new("obj:a").with_state("x", 1i64));

        let mut w2 = w1.clone();
        w2.objects
            .get_mut("obj:a")
            .unwrap()
            .state
            .insert("x".into(), Value::Int(42));
        w2.add(Object::new("obj:b").with_state("name", "new"));

        let diff = w1.diff(&w2);

        let json = serde_json::to_string(&diff).unwrap();
        let restored: WorldDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(diff, restored);
    }

    #[test]
    fn test_multiple_changes() {
        let mut w1 = World::new();
        w1.add(
            Object::new("obj:a")
                .with_state("hp", 100i64)
                .with_state("mp", 50i64),
        );
        w1.add(Object::new("obj:b").with_state("alive", true));
        w1.add(Object::new("obj:c").with_state("temp", "delete me"));

        let mut w2 = w1.clone();
        // Change obj:a hp
        w2.objects
            .get_mut("obj:a")
            .unwrap()
            .state
            .insert("hp".into(), Value::Int(80));
        // Change obj:b alive
        w2.objects
            .get_mut("obj:b")
            .unwrap()
            .state
            .insert("alive".into(), Value::Bool(false));
        // Remove obj:c
        w2.objects.remove("obj:c");
        // Add obj:d
        w2.add(Object::new("obj:d").with_state("new", true));

        let diff = w1.diff(&w2);

        assert_eq!(diff.state_changes.len(), 2);
        assert!(diff.state_changes.contains_key("obj:a"));
        assert!(diff.state_changes.contains_key("obj:b"));
        assert_eq!(diff.removed, vec!["obj:c".to_string()]);
        assert_eq!(diff.added.len(), 1);
        assert!(diff.added.contains_key("obj:d"));

        // Apply and verify roundtrip
        let mut result = w1.clone();
        result.apply_diff(&diff);
        assert_eq!(result.objects.len(), w2.objects.len());
        for (id, expected) in &w2.objects {
            let actual = result.objects.get(id).unwrap();
            assert_eq!(actual.state, expected.state);
        }
    }

    #[test]
    fn test_state_key_removal() {
        let mut w1 = World::new();
        w1.add(
            Object::new("obj:a")
                .with_state("keep", "yes")
                .with_state("remove", "bye"),
        );

        let mut w2 = w1.clone();
        w2.objects.get_mut("obj:a").unwrap().state.remove("remove");

        let diff = w1.diff(&w2);
        let changes = diff.state_changes.get("obj:a").unwrap();
        assert_eq!(changes.get("remove"), Some(&None));

        let mut result = w1.clone();
        result.apply_diff(&diff);
        assert_eq!(
            result.objects.get("obj:a").unwrap().state.get("keep"),
            Some(&Value::String("yes".into()))
        );
        assert!(!result
            .objects
            .get("obj:a")
            .unwrap()
            .state
            .contains_key("remove"));
    }
}
