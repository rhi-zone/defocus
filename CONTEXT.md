# Ubiquitous Language

Domain vocabulary for defocus. Use these terms precisely in code, docs, and conversations.

## World
_Avoid:_ state, container, environment

The simulation substrate: a collection of objects, a schedule queue, an event log, and an LLM provider. Everything in defocus lives inside a world. The world is the unit of replay, forking, and persistence.

## Object
_Avoid:_ entity, node, record, component

An identified entity in the world with state, handlers, an interface, optional children, and an optional prototype. Objects interact only through message passing — there is no direct field access from outside.

## Identity
_Avoid:_ ID, string, name

A string identifier with a namespace prefix convention: `proto:` for prototypes, `room:` for rooms, `local:` for transient objects. Not a bare string — the namespace prefix carries semantic meaning.

## Message
_Avoid:_ event, command, call, request

A verb + payload sent to an object. The sole interaction mechanism in defocus. Messages are dispatched, not called — the sender does not block. Distinct from Event (which is the post-dispatch log record).

## Verb
_Avoid:_ method, action, type, kind

The discriminating name of a message. Determines which handler on the target object is invoked. Not an HTTP verb, not a grammatical verb — just the message action name.

## Handler
_Avoid:_ function, method, callback

Two related things: (1) the Expr stored at `handlers[verb]` — the code that runs; (2) the verb→Expr binding in an object's handler table. Always specify which sense when ambiguous.

## Interface
_Avoid:_ API, methods, protocol

The list of verbs an object handles — the set of messages it accepts. Defines the object's protocol-level contract. Objects without a handler for a verb silently drop the message (unless they are Stubs).

## Prototype
_Avoid:_ parent class, base class, supertype

An optional parent object whose handler table is consulted when the target has no handler for a verb. Dynamic delegation: the message resolves against the prototype's handlers, but state modifications still apply to the target object, not the prototype. Not inheritance.

## Stub
_Avoid:_ placeholder, mock, empty object

A minimal object with an interface but no handlers. Messages sent to a stub are silently dropped. Used as a placeholder for objects not yet implemented.

## Expr / Expression
_Avoid:_ code, function, script

A value-as-code AST in Marinada (a JSON-serializable subset). Handlers, computations, and rules are all Exprs. An Expr is data that can be evaluated — it is not a Lua function or a string of source code.

## Perform
_Avoid:_ call, invoke, execute, function

The sole mutation boundary in Expr evaluation: `["perform", tag, ...]` generates an Effect. All other Expr forms are pure — they compute values but produce no side effects. Confusing Perform with a regular function call leads to expecting mutations from pure Exprs.

## Effect
_Avoid:_ side effect, result, output

A mutation or output produced by evaluating a Perform: SetState, Send, Reply, Schedule, Spawn, or Remove. Effects are the only way objects change state or communicate. The handler returns effects; the runtime applies them.

## Ref
_Avoid:_ ID, pointer, string reference

A capability pointer to another object: `Value::Ref { id, verbs }`. May be attenuated to a subset of verbs. JSON-serializable as `{ "$ref": id, "$verbs": [...] }`. Not a bare Identity string — a Ref carries access permissions.

## Attenuate
_Avoid:_ restrict, filter, limit

Narrowing a Ref to a subset of allowed verbs. A capability security pattern: you can share a Ref that only allows `read` without exposing `write`. The attenuated Ref cannot be "upgraded" by the recipient.

## Query
_Avoid:_ search, lookup, find

The `["query", filter-record]` Expr form: a linear scan of the live world filtering objects by state, interface, prototype, or children. Not a function call — a built-in Expr that runs against the world at evaluation time.

## Event
_Avoid:_ message (different concept)

The post-dispatch log record: target, verb, sender, replies. Written to the EventLog after a message is processed. An Event records what happened; a Message is what was sent. The EventLog is built from Events, not Messages.

## EventLog
_Avoid:_ audit log, history, journal

The append-only sequence of Events. The replay substrate: deterministically re-dispatching an EventLog against a fresh world reconstructs the simulation state. Not just a record — it is the canonical source of truth for world state.

## Replay
_Avoid:_ restore, reload, reconstruct

Re-dispatching an EventLog against a world to deterministically reconstruct state. Replay is deterministic: the same EventLog always produces the same world. Used for persistence, debugging, and forking.

## Fork
_Avoid:_ clone, copy, branch, checkpoint

Creating a branching world state by replaying the EventLog up to a chosen index, then truncating. The forked world diverges from that point. Distinct from cloning current state — Fork goes through replay, not a state snapshot.

## Tick
_Avoid:_ frame, step, timestamp

The simulation time unit used for scheduling and event ordering. The schedule maps tick→messages; events are ordered by tick. Not a wall-clock duration — an abstract simulation counter.

## WorldDiff
_Avoid:_ patch, delta, changeset

The structural diff between two world states (`World::diff` / `apply_diff`). Distinct from an EventLog: a WorldDiff is a structural comparison of two snapshots, not a causal record of what happened between them.
