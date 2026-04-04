use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The universal value type. Mirrors Marinada's value model.
/// Expressions are also Values — an array with a string first element is a call.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Record(BTreeMap<String, Value>),
    /// A capability reference to another object by ID.
    /// `verbs: None` means unrestricted; `Some(vec)` restricts to those verbs.
    Ref {
        id: String,
        verbs: Option<Vec<String>>,
    },
}

impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(n) => serializer.serialize_i64(*n),
            Value::Float(n) => serializer.serialize_f64(*n),
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(a) => a.serialize(serializer),
            Value::Record(r) => r.serialize(serializer),
            Value::Ref { id, verbs } => {
                use serde::ser::SerializeMap;
                let size = if verbs.is_some() { 2 } else { 1 };
                let mut map = serializer.serialize_map(Some(size))?;
                map.serialize_entry("$ref", id)?;
                if let Some(v) = verbs {
                    map.serialize_entry("$verbs", v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct ValueVisitor;

        impl<'de> de::Visitor<'de> for ValueVisitor {
            type Value = crate::value::Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid value")
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Null)
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Null)
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Bool(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Int(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Int(v as i64))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(crate::value::Value::Float(v))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(crate::value::Value::String(v.to_owned()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(crate::value::Value::String(v))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut vec = Vec::new();
                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }
                Ok(crate::value::Value::Array(vec))
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut btree = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, crate::value::Value>()? {
                    btree.insert(key, value);
                }
                // If the record has "$ref" with a string value, treat as Ref
                if btree.contains_key("$ref") {
                    if let Some(crate::value::Value::String(id)) = btree.remove("$ref") {
                        let verbs = btree.remove("$verbs").and_then(|v| {
                            if let crate::value::Value::Array(arr) = v {
                                let strs: Vec<String> = arr
                                    .into_iter()
                                    .filter_map(|item| {
                                        if let crate::value::Value::String(s) = item {
                                            Some(s)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                if strs.is_empty() {
                                    None
                                } else {
                                    Some(strs)
                                }
                            } else {
                                None
                            }
                        });
                        // Only treat as Ref if no other keys remain
                        if btree.is_empty() {
                            return Ok(crate::value::Value::Ref { id, verbs });
                        }
                        // Put keys back
                        btree.insert("$ref".to_string(), crate::value::Value::String(id));
                        if let Some(v) = verbs {
                            btree.insert(
                                "$verbs".to_string(),
                                crate::value::Value::Array(
                                    v.into_iter().map(crate::value::Value::String).collect(),
                                ),
                            );
                        }
                    }
                }
                Ok(crate::value::Value::Record(btree))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Record(r) => !r.is_empty(),
            Value::Ref { .. } => true,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_ref_id(&self) -> Option<&str> {
        match self {
            Value::Ref { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn ref_verbs(&self) -> Option<&[String]> {
        match self {
            Value::Ref {
                verbs: Some(v), ..
            } => Some(v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_record(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Record(r) => Some(r),
            _ => None,
        }
    }

    pub fn get_in(&self, path: &[&str]) -> Option<&Value> {
        let mut current = self;
        for key in path {
            match current {
                Value::Record(r) => current = r.get(*key)?,
                _ => return None,
            }
        }
        Some(current)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Array(_) | Value::Record(_) => {
                write!(f, "{}", serde_json::to_string(self).unwrap_or_default())
            }
            Value::Ref { id, verbs: None } => write!(f, "<ref:{id}>"),
            Value::Ref {
                id,
                verbs: Some(v),
            } => write!(f, "<ref:{id}[{}]>", v.join(",")),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Float(n)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}
