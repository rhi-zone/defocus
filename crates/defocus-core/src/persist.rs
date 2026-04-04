use crate::world::World;

/// Minimal persistence backend. Borrowed from reincarnate's SaveBackend pattern.
/// Implementations: localStorage, OPFS, filesystem, SQLite, in-memory, etc.
pub trait SaveBackend {
    fn load(&self, key: &str) -> Option<String>;
    fn save(&mut self, key: &str, value: &str);
    fn remove(&mut self, key: &str);
    fn list(&self) -> Vec<String>;
}

/// In-memory backend for testing and embedded use.
#[derive(Default, Debug)]
pub struct MemoryBackend {
    store: std::collections::BTreeMap<String, String>,
}

impl SaveBackend for MemoryBackend {
    fn load(&self, key: &str) -> Option<String> {
        self.store.get(key).cloned()
    }
    fn save(&mut self, key: &str, value: &str) {
        self.store.insert(key.to_string(), value.to_string());
    }
    fn remove(&mut self, key: &str) {
        self.store.remove(key);
    }
    fn list(&self) -> Vec<String> {
        self.store.keys().cloned().collect()
    }
}

/// Fan-out writes to multiple backends.
pub struct Tee<A: SaveBackend, B: SaveBackend> {
    pub a: A,
    pub b: B,
}

impl<A: SaveBackend, B: SaveBackend> SaveBackend for Tee<A, B> {
    fn load(&self, key: &str) -> Option<String> {
        self.a.load(key).or_else(|| self.b.load(key))
    }
    fn save(&mut self, key: &str, value: &str) {
        self.a.save(key, value);
        self.b.save(key, value);
    }
    fn remove(&mut self, key: &str) {
        self.a.remove(key);
        self.b.remove(key);
    }
    fn list(&self) -> Vec<String> {
        let mut keys = self.a.list();
        for k in self.b.list() {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys
    }
}

/// Keep last N saves per prefix, pruning oldest.
pub struct Rolling<B: SaveBackend> {
    pub inner: B,
    pub max: usize,
    pub prefix: String,
}

impl<B: SaveBackend> SaveBackend for Rolling<B> {
    fn load(&self, key: &str) -> Option<String> {
        self.inner.load(key)
    }

    fn save(&mut self, key: &str, value: &str) {
        self.inner.save(key, value);
        // Prune if we have too many prefixed slots
        let slots: Vec<String> = self
            .inner
            .list()
            .into_iter()
            .filter(|k| k.starts_with(&self.prefix))
            .collect();
        if slots.len() > self.max {
            // Remove oldest (lowest numbered)
            let mut numbered: Vec<(usize, String)> = slots
                .into_iter()
                .filter_map(|k| {
                    k.strip_prefix(&self.prefix)
                        .and_then(|n| n.parse::<usize>().ok())
                        .map(|n| (n, k))
                })
                .collect();
            numbered.sort_by_key(|(n, _)| *n);
            while numbered.len() > self.max {
                if let Some((_, key)) = numbered.first() {
                    self.inner.remove(key);
                }
                numbered.remove(0);
            }
        }
    }

    fn remove(&mut self, key: &str) {
        self.inner.remove(key);
    }

    fn list(&self) -> Vec<String> {
        self.inner.list()
    }
}

/// Convenience: save/load a World to a backend.
impl World {
    pub fn save_to(&self, backend: &mut dyn SaveBackend, key: &str) {
        let json = self.to_json();
        backend.save(key, &json.to_string());
    }

    pub fn load_from(backend: &dyn SaveBackend, key: &str) -> Option<Result<World, String>> {
        let data = backend.load(key)?;
        let json: serde_json::Value = match serde_json::from_str(&data) {
            Ok(j) => j,
            Err(e) => return Some(Err(e.to_string())),
        };
        Some(World::from_json(json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use crate::world::Object;

    #[test]
    fn memory_backend_round_trip() {
        let mut backend = MemoryBackend::default();
        let mut world = World::new();
        world.add(
            Object::new("local:test")
                .with_state("x", Value::Int(42))
                .with_ref("friend", "local:other"),
        );

        world.save_to(&mut backend, "save-1");
        let loaded = World::load_from(&backend, "save-1").unwrap().unwrap();

        assert_eq!(loaded.objects["local:test"].state["x"], Value::Int(42));
        assert_eq!(
            loaded.objects["local:test"].state["friend"],
            Value::Ref {
                id: "local:other".into(),
                verbs: None
            }
        );
    }

    #[test]
    fn tee_writes_to_both() {
        let mut tee = Tee {
            a: MemoryBackend::default(),
            b: MemoryBackend::default(),
        };
        tee.save("key", "value");
        assert_eq!(tee.a.load("key"), Some("value".into()));
        assert_eq!(tee.b.load("key"), Some("value".into()));
    }

    #[test]
    fn rolling_prunes_oldest() {
        let mut rolling = Rolling {
            inner: MemoryBackend::default(),
            max: 2,
            prefix: "save-".into(),
        };
        rolling.save("save-0", "first");
        rolling.save("save-1", "second");
        rolling.save("save-2", "third");

        // save-0 should be pruned
        assert!(rolling.load("save-0").is_none());
        assert_eq!(rolling.load("save-1"), Some("second".into()));
        assert_eq!(rolling.load("save-2"), Some("third".into()));
    }
}
