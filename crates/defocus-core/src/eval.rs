use crate::llm::LlmProvider;
use crate::value::Value;
use crate::world::{Effect, Expr, Identity, Message, Object};
use std::collections::BTreeMap;

struct Env<'a> {
    bindings: Vec<(String, Value)>,
    effects: Vec<Effect>,
    llm: Option<&'a dyn LlmProvider>,
}

impl<'a> Env<'a> {
    fn new(llm: Option<&'a dyn LlmProvider>) -> Self {
        Env {
            bindings: Vec::new(),
            effects: Vec::new(),
            llm,
        }
    }

    fn bind(&mut self, name: String, value: Value) {
        self.bindings.push((name, value));
    }

    fn get(&self, name: &str) -> Value {
        for (k, v) in self.bindings.iter().rev() {
            if k == name {
                return v.clone();
            }
        }
        Value::Null
    }

    fn push_scope(&self) -> usize {
        self.bindings.len()
    }

    fn pop_scope(&mut self, mark: usize) {
        self.bindings.truncate(mark);
    }
}

pub fn eval_handler(
    handler: &Expr,
    payload: &Value,
    state: &Value,
    self_id: &str,
    sender: Option<&str>,
) -> Vec<Effect> {
    eval_handler_with_llm(handler, payload, state, self_id, sender, None)
}

pub fn eval_handler_with_llm(
    handler: &Expr,
    payload: &Value,
    state: &Value,
    self_id: &str,
    sender: Option<&str>,
    llm: Option<&dyn LlmProvider>,
) -> Vec<Effect> {
    let mut env = Env::new(llm);
    env.bind(
        "self".into(),
        Value::Ref {
            id: self_id.to_string(),
            verbs: None,
        },
    );
    env.bind(
        "sender".into(),
        match sender {
            Some(id) => Value::Ref {
                id: id.to_string(),
                verbs: None,
            },
            None => Value::Null,
        },
    );
    env.bind("payload".into(), payload.clone());
    env.bind("state".into(), state.clone());
    eval(handler, &mut env);
    env.effects
}

fn eval(expr: &Expr, env: &mut Env) -> Value {
    match expr {
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) => expr.clone(),
        Value::String(_) | Value::Ref { .. } => expr.clone(),
        Value::Record(r) => {
            let mut result = BTreeMap::new();
            for (k, v) in r {
                result.insert(k.clone(), eval(v, env));
            }
            Value::Record(result)
        }
        Value::Array(arr) if arr.is_empty() => Value::Array(vec![]),
        Value::Array(arr) => {
            let Some(op) = arr[0].as_str() else {
                // Not a call — evaluate as array literal
                return Value::Array(arr.iter().map(|v| eval(v, env)).collect());
            };
            let args = &arr[1..];
            eval_call(op, args, env)
        }
    }
}

fn eval_call(op: &str, args: &[Value], env: &mut Env) -> Value {
    match op {
        // Variable access
        "get" => {
            let key = eval(&args[0], env);
            match key.as_str() {
                Some(name) => env.get(name),
                None => Value::Null,
            }
        }

        // Nested access: ["get-in", expr, key1, key2, ...]
        "get-in" => {
            let mut current = eval(&args[0], env);
            for key_expr in &args[1..] {
                let key = eval(key_expr, env);
                current = match (&current, &key) {
                    (Value::Record(r), Value::String(k)) => {
                        r.get(k).cloned().unwrap_or(Value::Null)
                    }
                    (Value::Array(a), Value::Int(i)) => {
                        a.get(*i as usize).cloned().unwrap_or(Value::Null)
                    }
                    _ => Value::Null,
                };
            }
            current
        }

        // Control flow
        "if" => {
            let cond = eval(&args[0], env);
            if cond.is_truthy() {
                eval(&args[1], env)
            } else if args.len() > 2 {
                eval(&args[2], env)
            } else {
                Value::Null
            }
        }

        "do" => {
            let mut result = Value::Null;
            for arg in args {
                result = eval(arg, env);
            }
            result
        }

        "let" => {
            // ["let", name, value, body]
            let name = args[0].as_str().unwrap_or("_").to_string();
            let value = eval(&args[1], env);
            let mark = env.push_scope();
            env.bind(name, value);
            let result = eval(&args[2], env);
            env.pop_scope(mark);
            result
        }

        // Arithmetic
        "+" => numeric_binop(args, env, |a, b| a + b, |a, b| a + b),
        "-" => numeric_binop(args, env, |a, b| a - b, |a, b| a - b),
        "*" => numeric_binop(args, env, |a, b| a * b, |a, b| a * b),
        "/" => numeric_binop(
            args,
            env,
            |a, b| if b != 0 { a / b } else { 0 },
            |a, b| a / b,
        ),

        // Comparison
        "=" => {
            let a = eval(&args[0], env);
            let b = eval(&args[1], env);
            Value::Bool(a == b)
        }
        "!=" => {
            let a = eval(&args[0], env);
            let b = eval(&args[1], env);
            Value::Bool(a != b)
        }
        "<" => compare_op(args, env, |o| o.is_lt()),
        ">" => compare_op(args, env, |o| o.is_gt()),
        "<=" => compare_op(args, env, |o| o.is_le()),
        ">=" => compare_op(args, env, |o| o.is_ge()),

        // Logic
        "and" => {
            let a = eval(&args[0], env);
            if !a.is_truthy() {
                a
            } else {
                eval(&args[1], env)
            }
        }
        "or" => {
            let a = eval(&args[0], env);
            if a.is_truthy() {
                a
            } else {
                eval(&args[1], env)
            }
        }
        "not" => {
            let a = eval(&args[0], env);
            Value::Bool(!a.is_truthy())
        }

        // Data constructors
        "array" => Value::Array(args.iter().map(|v| eval(v, env)).collect()),
        "record" => {
            let mut r = BTreeMap::new();
            for pair in args.chunks(2) {
                if let Some(key) = eval(&pair[0], env).as_str().map(String::from) {
                    let value = pair.get(1).map(|v| eval(v, env)).unwrap_or(Value::Null);
                    r.insert(key, value);
                }
            }
            Value::Record(r)
        }

        // String
        "concat" => {
            let mut result = String::new();
            for arg in args {
                result.push_str(&eval(arg, env).to_string());
            }
            Value::String(result)
        }

        // Pattern matching: ["match", scrutinee, [pattern, body], ...]
        "match" => {
            let scrutinee = eval(&args[0], env);
            for arm in &args[1..] {
                let Some(arm_arr) = arm.as_array() else {
                    continue;
                };
                if arm_arr.len() != 2 {
                    continue;
                }
                let mark = env.push_scope();
                if match_pattern(&arm_arr[0], &scrutinee, env) {
                    let result = eval(&arm_arr[1], env);
                    env.pop_scope(mark);
                    return result;
                }
                env.pop_scope(mark);
            }
            Value::Null
        }

        // Functions
        "fn" => {
            // ["fn", [params...], body] → ["$fn", [params...], body]
            if args.len() < 2 {
                return Value::Null;
            }
            let params = args[0].clone();
            let body = args[1].clone();
            Value::Array(vec![Value::String("$fn".into()), params, body])
        }

        "call" => {
            // ["call", fn-expr, arg1, arg2, ...]
            if args.is_empty() {
                return Value::Null;
            }
            let func = eval(&args[0], env);
            let Some(fn_arr) = func.as_array() else {
                return Value::Null;
            };
            if fn_arr.len() != 3 || fn_arr[0].as_str() != Some("$fn") {
                return Value::Null;
            }
            let Some(params) = fn_arr[1].as_array() else {
                return Value::Null;
            };
            let body = &fn_arr[2];

            // Evaluate arguments
            let evaluated_args: Vec<Value> = args[1..].iter().map(|a| eval(a, env)).collect();

            // Push scope and bind params
            let mark = env.push_scope();
            for (i, param) in params.iter().enumerate() {
                if let Some(name) = param.as_str() {
                    let value = evaluated_args.get(i).cloned().unwrap_or(Value::Null);
                    env.bind(name.to_string(), value);
                }
            }
            let result = eval(body, env);
            env.pop_scope(mark);
            result
        }

        // Capability attenuation
        "attenuate" => {
            // ["attenuate", ref-expr, ["verb1", "verb2"]]
            if args.len() < 2 {
                return Value::Null;
            }
            let ref_val = eval(&args[0], env);
            let verbs_val = eval(&args[1], env);
            let Some(new_verbs_arr) = verbs_val.as_array() else {
                return Value::Null;
            };
            let new_verbs: Vec<String> = new_verbs_arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            match ref_val {
                Value::Ref {
                    id,
                    verbs: existing,
                } => {
                    let final_verbs = match existing {
                        None => new_verbs,
                        Some(existing_verbs) => {
                            // Intersection: only keep verbs in both lists
                            new_verbs
                                .into_iter()
                                .filter(|v| existing_verbs.contains(v))
                                .collect()
                        }
                    };
                    Value::Ref {
                        id,
                        verbs: Some(final_verbs),
                    }
                }
                _ => Value::Null,
            }
        }

        // Effects
        "perform" => {
            let tag = args[0].as_str().unwrap_or("unknown");
            match tag {
                "set" => {
                    if args.len() >= 3 {
                        let key = eval(&args[1], env).as_str().unwrap_or("").to_string();
                        let value = eval(&args[2], env);
                        env.effects.push(Effect::SetState { key, value });
                    }
                }
                "send" => {
                    if args.len() >= 4 {
                        let target = eval(&args[1], env);
                        // Extract both the ID and verb filter from the ref
                        let (to, allowed_verbs): (Identity, Option<Vec<String>>) = match &target {
                            Value::Ref { id, verbs } => {
                                (id.clone(), verbs.clone())
                            }
                            _ => {
                                let id = target.as_str().unwrap_or("").to_string();
                                (id, None)
                            }
                        };
                        let verb = eval(&args[2], env).as_str().unwrap_or("").to_string();
                        let payload = eval(&args[3], env);
                        env.effects.push(Effect::Send {
                            to,
                            allowed_verbs,
                            message: Message { verb, payload },
                        });
                    }
                }
                "reply" => {
                    if args.len() >= 2 {
                        let value = eval(&args[1], env);
                        env.effects.push(Effect::Reply { value });
                    }
                }
                "remove" => {
                    if args.len() >= 2 {
                        let target = eval(&args[1], env);
                        let id: Identity = target
                            .as_ref_id()
                            .or_else(|| target.as_str())
                            .unwrap_or("")
                            .to_string();
                        env.effects.push(Effect::Remove { id });
                    }
                }
                "schedule" => {
                    // ["perform", "schedule", tick-expr, ref-or-id, verb, payload]
                    if args.len() >= 5 {
                        let at = eval(&args[1], env)
                            .as_i64()
                            .unwrap_or(0) as u64;
                        let target = eval(&args[2], env);
                        let to: Identity = target
                            .as_ref_id()
                            .or_else(|| target.as_str())
                            .unwrap_or("")
                            .to_string();
                        let verb = eval(&args[3], env).as_str().unwrap_or("").to_string();
                        let payload = eval(&args[4], env);
                        env.effects.push(Effect::Schedule {
                            at,
                            to,
                            message: Message { verb, payload },
                        });
                    }
                }
                "spawn" => {
                    if args.len() >= 3 {
                        let target = eval(&args[1], env);
                        let id: Identity = target
                            .as_ref_id()
                            .or_else(|| target.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Don't fully evaluate the spec — handlers are stored
                        // as unevaluated expressions, interface is data.
                        // Only evaluate state values (for computed initial state).
                        let spec = &args[2];
                        if let Some(spec_rec) = spec.as_record() {
                            let state = spec_rec
                                .get("state")
                                .and_then(|v| v.as_record())
                                .map(|r| r.iter().map(|(k, v)| (k.clone(), eval(v, env))).collect())
                                .unwrap_or_default();
                            let handlers = spec_rec
                                .get("handlers")
                                .and_then(|v| v.as_record())
                                .cloned()
                                .unwrap_or_default();
                            let interface = spec_rec
                                .get("interface")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let prototype = spec_rec
                                .get("prototype")
                                .and_then(|v| v.as_ref_id().or_else(|| v.as_str()))
                                .map(String::from);
                            let object = Object {
                                id: id.clone(),
                                state,
                                handlers,
                                interface,
                                children: Vec::new(),
                                prototype,
                            };
                            env.effects.push(Effect::Spawn { object });
                            return Value::Ref {
                                id,
                                verbs: None,
                            };
                        }
                    }
                }
                _ => {}
            }
            Value::Null
        }

        // LLM call: ["llm", prompt-expr]
        "llm" => {
            if let Some(provider) = env.llm {
                let prompt = eval(&args[0], env);
                let prompt_str = prompt.to_string();
                match provider.complete(&prompt_str) {
                    Ok(response) => Value::String(response),
                    Err(_) => Value::Null,
                }
            } else {
                Value::Null
            }
        }

        // Unknown op — return null
        _ => Value::Null,
    }
}

fn numeric_binop(
    args: &[Value],
    env: &mut Env,
    int_op: fn(i64, i64) -> i64,
    float_op: fn(f64, f64) -> f64,
) -> Value {
    let a = eval(&args[0], env);
    let b = eval(&args[1], env);
    match (&a, &b) {
        (Value::Int(a), Value::Int(b)) => Value::Int(int_op(*a, *b)),
        _ => {
            let a = a.as_f64().unwrap_or(0.0);
            let b = b.as_f64().unwrap_or(0.0);
            Value::Float(float_op(a, b))
        }
    }
}

fn compare_op(args: &[Value], env: &mut Env, pred: fn(std::cmp::Ordering) -> bool) -> Value {
    let a = eval(&args[0], env);
    let b = eval(&args[1], env);
    let ord = match (&a, &b) {
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        _ => {
            let a = a.as_f64().unwrap_or(0.0);
            let b = b.as_f64().unwrap_or(0.0);
            a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
        }
    };
    Value::Bool(pred(ord))
}

fn match_pattern(pattern: &Value, scrutinee: &Value, env: &mut Env) -> bool {
    match pattern {
        // "_" matches anything
        Value::String(s) if s == "_" => true,
        // String starting with $ = binding
        Value::String(s) if s.starts_with('$') => {
            env.bind(s.clone(), scrutinee.clone());
            true
        }
        // Literal match
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) => pattern == scrutinee,
        // String literal match (uppercase or non-alpha)
        Value::String(_) => pattern == scrutinee,
        // Array pattern — match element-wise
        Value::Array(pat_arr) => {
            let Some(scrut_arr) = scrutinee.as_array() else {
                return false;
            };
            if pat_arr.len() != scrut_arr.len() {
                return false;
            }
            pat_arr
                .iter()
                .zip(scrut_arr.iter())
                .all(|(p, s)| match_pattern(p, s, env))
        }
        // Record pattern — all keys in pattern must match
        Value::Record(pat_rec) => {
            let Some(scrut_rec) = scrutinee.as_record() else {
                return false;
            };
            pat_rec.iter().all(|(k, p)| {
                scrut_rec
                    .get(k)
                    .map(|s| match_pattern(p, s, env))
                    .unwrap_or(false)
            })
        }
        // Ref literal match
        Value::Ref { .. } => pattern == scrutinee,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn val(j: serde_json::Value) -> Value {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn test_arithmetic() {
        let effects = eval_handler(
            &val(json!(["+", 1, 2])),
            &Value::Null,
            &Value::Null,
            "",
            None,
        );
        assert!(effects.is_empty());
    }

    #[test]
    fn test_set_effect() {
        let handler = val(json!(["perform", "set", "open", true]));
        let effects = eval_handler(&handler, &Value::Null, &Value::Null, "", None);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::SetState { key, value } => {
                assert_eq!(key, "open");
                assert_eq!(*value, Value::Bool(true));
            }
            _ => panic!("expected SetState"),
        }
    }

    #[test]
    fn test_conditional_handler() {
        let handler = val(json!([
            "if",
            ["=", ["get", "payload"], "open"],
            ["perform", "set", "open", true],
            ["perform", "set", "open", false]
        ]));

        let effects = eval_handler(
            &handler,
            &Value::String("open".into()),
            &Value::Null,
            "",
            None,
        );
        match &effects[0] {
            Effect::SetState { value, .. } => {
                assert_eq!(*value, Value::Bool(true));
            }
            _ => panic!("expected SetState"),
        }

        let effects = eval_handler(
            &handler,
            &Value::String("close".into()),
            &Value::Null,
            "",
            None,
        );
        match &effects[0] {
            Effect::SetState { value, .. } => {
                assert_eq!(*value, Value::Bool(false));
            }
            _ => panic!("expected SetState"),
        }
    }

    #[test]
    fn test_match_pattern() {
        let handler = val(json!([
            "match",
            ["get", "payload"],
            ["open", ["perform", "set", "open", true]],
            ["close", ["perform", "set", "open", false]],
            ["_", null]
        ]));

        let effects = eval_handler(
            &handler,
            &Value::String("open".into()),
            &Value::Null,
            "",
            None,
        );
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::SetState { value, .. } => assert_eq!(*value, Value::Bool(true)),
            _ => panic!("expected SetState"),
        }
    }

    #[test]
    fn test_send_effect() {
        let handler = val(json!([
            "do",
            ["perform", "set", "open", true],
            ["perform", "send", "local:frame", "opened", null]
        ]));

        let effects = eval_handler(&handler, &Value::Null, &Value::Null, "", None);
        assert_eq!(effects.len(), 2);
        match &effects[1] {
            Effect::Send {
                to,
                allowed_verbs,
                message,
            } => {
                assert_eq!(to, "local:frame");
                assert_eq!(message.verb, "opened");
                assert!(allowed_verbs.is_none());
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn test_fn_simple() {
        // ["let", "add", ["fn", ["a", "b"], ["+", ["get", "a"], ["get", "b"]]], ["call", ["get", "add"], 3, 4]]
        let handler = val(json!([
            "let",
            "add",
            ["fn", ["a", "b"], ["+", ["get", "a"], ["get", "b"]]],
            ["call", ["get", "add"], 3, 4]
        ]));
        let mut env = Env::new(None);
        let result = eval(&handler, &mut env);
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_fn_no_args() {
        // ["call", ["fn", [], 42]]
        let handler = val(json!(["call", ["fn", [], 42]]));
        let mut env = Env::new(None);
        let result = eval(&handler, &mut env);
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_fn_as_value() {
        // Function stored in state, retrieved and called
        let handler = val(json!(["call", ["get", "state"], 10, 20]));
        let state = val(json!([
            "$fn",
            ["a", "b"],
            ["+", ["get", "a"], ["get", "b"]]
        ]));
        let mut env = Env::new(None);
        env.bind("state".into(), state);
        let result = eval(&handler, &mut env);
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn test_fn_nested_calls() {
        // ["let", "double", ["fn", ["x"], ["+", ["get", "x"], ["get", "x"]]],
        //   ["call", ["get", "double"], ["call", ["get", "double"], 3]]]
        let handler = val(json!([
            "let",
            "double",
            ["fn", ["x"], ["+", ["get", "x"], ["get", "x"]]],
            ["call", ["get", "double"], ["call", ["get", "double"], 3]]
        ]));
        let mut env = Env::new(None);
        let result = eval(&handler, &mut env);
        assert_eq!(result, Value::Int(12));
    }

    #[test]
    fn test_get_in_state() {
        let handler = val(json!(["get-in", ["get", "state"], "health", "current"]));
        let state = val(json!({
            "health": { "current": 75 }
        }));

        let effects = eval_handler(&handler, &Value::Null, &state, "", None);
        assert!(effects.is_empty()); // No effects, just reads
    }
}
