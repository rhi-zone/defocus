# defocus

World substrate for interactive narrative, IF, and stateful simulations.

## What is defocus?

defocus is a protocol-level world model where everything is an object, all interaction is message passing, and rules are structured data (ASTs, not text). The same world file runs on multiple runtimes: Rust (server/CLI), TypeScript (browser/Node), with WASM and Lua via Crescent planned. It replaces the ad-hoc state management that every IF/narrative tool reinvents.

## Quick Example

### TypeScript

```typescript
import { WorldBuilder } from "defocus-core";

const world = new WorldBuilder()
  .object("local:room")
    .state("description", "A dusty room with a heavy door.")
    .ref("door", "local:door")
    .handler("look", ["perform", "reply", ["get-in", ["get", "state"], "description"]])
    .done()
  .object("local:door")
    .state("open", false)
    .handler("open", [
      "do",
      ["perform", "set", "open", true],
      ["perform", "reply", "The door creaks open."],
    ])
    .done()
  .build();

world.send("local:room", { verb: "look", payload: null });
let replies = world.drain(100);
// => ["A dusty room with a heavy door."]

world.send("local:door", { verb: "open", payload: null });
replies = world.drain(100);
// => ["The door creaks open."]
```

### Rust

```rust
use defocus_core::world::{Object, World, Message};
use defocus_core::value::Value;

let mut world = World::new();

world.add(
    Object::new("local:room")
        .with_state("description", "A dusty room with a heavy door.")
        .with_ref("door", "local:door")
        .with_handler("look", serde_json::from_value(serde_json::json!(
            ["perform", "reply", ["get-in", ["get", "state"], "description"]]
        )).unwrap())
);

world.add(
    Object::new("local:door")
        .with_state("open", Value::Bool(false))
        .with_handler("open", serde_json::from_value(serde_json::json!([
            "do",
            ["perform", "set", "open", true],
            ["perform", "reply", "The door creaks open."]
        ])).unwrap())
);

world.send("local:room".into(), Message { verb: "look".into(), payload: Value::Null });
let replies = world.drain(100);
// => [Value::String("A dusty room with a heavy door.")]
```

## Features

- **Object-message architecture** -- everything is an object, all interaction is message passing
- **Rules as data** -- handlers are JSON ASTs, not source text; diffable, serializable, visually editable
- **Prototype inheritance** -- objects delegate to prototypes; handler resolution walks the chain
- **Capability-based refs** -- `$ref` values with optional `$verbs` for attenuated access
- **Fluent builder DSL** (TypeScript) -- `WorldBuilder` / `ObjectBuilder` for ergonomic world construction
- **Expression evaluator** -- Marinada subset: arithmetic, logic, control flow, closures, pattern matching
- **Algebraic effects** -- `perform` expressions produce effects (set state, send messages, reply, spawn, remove, schedule)
- **World queries** -- `query` op filters objects by state, interface, prototype, children, has-state
- **LLM integration** -- `llm` op calls a pluggable provider from within handlers; outputs logged for deterministic replay
- **Event logging** -- optional event log records every message dispatch and reply
- **Branching / fork** -- fork world state at any point in the event log
- **Replay** -- re-dispatch a log against a world for deterministic reconstruction
- **Persistence** -- `SaveBackend` trait with `MemoryBackend`, `Tee`, `Rolling`; browser backends for `localStorage` and `IndexedDB`
- **World diffing** -- structural diff between world snapshots
- **Scheduled messages** -- `schedule` effect delivers messages at future ticks
- **JSON serialization** -- worlds round-trip through a versioned JSON format
- **Interconnect adapter** (`defocus-interconnect` crate) -- expose a world as an Interconnect authority

## Architecture

A defocus world consists of:

- **Values** -- null, bool, number, string, array, record, ref. Refs (`{ $ref: id }`) are capabilities pointing to other objects, optionally attenuated with `$verbs`.
- **Objects** -- identified by string IDs. Each has state (key-value), handlers (verb -> expression), an interface (list of verbs), children, and an optional prototype.
- **Messages** -- a verb string + a payload value, sent to an object ID.
- **Evaluator** -- walks expression ASTs, producing effects. Handlers receive `self`, `sender`, `payload`, and `state` bindings.
- **Effects** -- set state, send message, reply, spawn object, remove object, schedule future message.
- **Event log** -- append-only record of dispatched messages and their replies.
- **Persistence** -- composable backends: memory, tee (fan-out), rolling (N most recent snapshots), localStorage, IndexedDB.

## Expression Language

Expressions are JSON values. An array with a string first element is a function call.

### Variables

| Op | Form | Description |
|----|------|-------------|
| `get` | `["get", name]` | Read a binding (`self`, `sender`, `payload`, `state`, or user-defined) |
| `get-in` | `["get-in", expr, key, ...]` | Nested access into records/arrays |
| `let` | `["let", name, value, body]` | Bind a value in a scope |
| `let-fn` | `["let-fn", name, [params], body, cont]` | Bind a recursive function |

### Control Flow

| Op | Form | Description |
|----|------|-------------|
| `if` | `["if", cond, then, else?]` | Conditional |
| `do` | `["do", expr, ...]` | Sequential execution |
| `match` | `["match", scrutinee, [pattern, body], ...]` | Pattern matching |
| `while` | `["while", cond, body]` | Loop with condition |
| `for` | `["for", var, iterable, body]` | Iterate over array |
| `loop` | `["loop", body]` | Infinite loop (exit with `break`) |
| `break` | `["break", value?]` | Exit loop |
| `return` | `["return", value?]` | Early return from function |

### Arithmetic

| Op | Form | Description |
|----|------|-------------|
| `+` `-` `*` `/` | `[op, a, b]` | Basic arithmetic |
| `mod` | `["mod", a, b]` | Modulo |
| `floor` `ceil` `round` `abs` | `[op, n]` | Rounding/absolute value |
| `min` `max` | `[op, a, b]` | Min/max |

### Comparison & Logic

| Op | Form | Description |
|----|------|-------------|
| `=` `!=` `<` `>` `<=` `>=` | `[op, a, b]` | Comparison |
| `and` `or` `not` | `[op, ...]` | Short-circuit logic |

### Strings

| Op | Form | Description |
|----|------|-------------|
| `concat` | `["concat", ...]` | Concatenate to string |
| `split` | `["split", str, sep]` | Split string |
| `join` | `["join", arr, sep]` | Join array to string |
| `trim` | `["trim", str]` | Trim whitespace |
| `upper` `lower` | `[op, str]` | Case conversion |
| `starts-with` `ends-with` | `[op, str, fix]` | Prefix/suffix check |
| `slice` | `["slice", str, start, end?]` | Substring (also works on arrays) |

### Arrays

| Op | Form | Description |
|----|------|-------------|
| `array` | `["array", ...]` | Construct array |
| `length` | `["length", arr]` | Length of array or string |
| `nth` | `["nth", arr, idx]` | Index into array |
| `push` | `["push", arr, val]` | Append (returns new array) |
| `map` | `["map", arr, fn]` | Map over array |
| `filter` | `["filter", arr, fn]` | Filter array |
| `reduce` | `["reduce", arr, fn, init]` | Fold array |
| `flat` | `["flat", arr]` | Flatten one level |
| `sort` | `["sort", arr]` | Sort (numeric or lexicographic) |
| `reverse` | `["reverse", arr]` | Reverse array |
| `range` | `["range", start, end]` | Integer range [start, end) |

### Records

| Op | Form | Description |
|----|------|-------------|
| `record` | `["record", k1, v1, k2, v2, ...]` | Construct record |
| `keys` | `["keys", rec]` | Get keys |
| `values` | `["values", rec]` | Get values |
| `has` | `["has", rec, key]` | Key existence check |
| `set-in` | `["set-in", rec, key, val]` | Set key (returns new record) |
| `remove-key` | `["remove-key", rec, key]` | Remove key (returns new record) |

### Functions

| Op | Form | Description |
|----|------|-------------|
| `fn` | `["fn", [params], body]` | Create closure |
| `call` | `["call", fn, arg, ...]` | Call a closure |

### Type Operations

| Op | Form | Description |
|----|------|-------------|
| `type` | `["type", val]` | Returns type name string |
| `is` | `["is", typename, val]` | Type check (null, bool, int, float, string, array, record, ref) |
| `try` | `["try", expr, fallback]` | Null coalescing |

### Capabilities

| Op | Form | Description |
|----|------|-------------|
| `attenuate` | `["attenuate", ref, ["verb1", ...]]` | Restrict a ref to specific verbs |

### World Queries

| Op | Form | Description |
|----|------|-------------|
| `query` | `["query", { state?, interface?, prototype?, "children-of"?, "has-state"? }]` | Find objects matching criteria |

### LLM

| Op | Form | Description |
|----|------|-------------|
| `llm` | `["llm", prompt]` | Call the world's LLM provider; returns string or null |

## Effects

Effects are produced by `["perform", tag, ...]` expressions.

| Effect | Form | Description |
|--------|------|-------------|
| `set` | `["perform", "set", key, value]` | Set a key on the current object's state |
| `send` | `["perform", "send", target, verb, payload]` | Send a message to another object (target can be a ref or string ID) |
| `reply` | `["perform", "reply", value]` | Return a value to the caller |
| `schedule` | `["perform", "schedule", tick, target, verb, payload]` | Schedule a message for a future tick |
| `spawn` | `["perform", "spawn", id, { state?, handlers?, interface?, prototype? }]` | Create a new object at runtime |
| `remove` | `["perform", "remove", target]` | Remove an object from the world |

## Runtimes

| Runtime | Status | Package |
|---------|--------|---------|
| **Rust** | Implemented | `defocus-core` crate |
| **TypeScript** | Implemented | `defocus-core` (npm, `ts/`) |
| **WASM** | Planned | -- |
| **Lua (Crescent)** | Planned | -- |

## Prior Art

- **LambdaMOO** -- the original object-message MUD substrate; defocus modernizes this model with structured rules, JSON serialization, and capability-based security.
- **Lotus/Viwo** -- the direct ancestor within this ecosystem; a persistent multiplayer MOO engine that was decomposed into ecosystem primitives, with defocus inheriting the world model.
- **existence** -- independently invented the text-as-rendering-layer architecture and observation-source/prose-compositor pattern that defocus adopts for its text output model.

## License

TBD
