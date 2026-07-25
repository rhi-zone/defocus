# CLAUDE.md

Behavioral rules for Claude Code in the defocus repository.

## Project Overview

World substrate for interactive narrative, IF, and stateful simulations

Part of the [rhi ecosystem](https://rhi.zone).

## Origin

**defocus** is a world substrate for interactive narrative, IF, and stateful simulations. The name comes from the core design principle: the world exists at the level of detail the story needs, no more. Objects are stubs until observed — like a camera that hasn't resolved the background yet.

### What problem it solves

Every IF/narrative game tool (Twine, Inform 7, SillyTavern, the adult IF games like DoL/TiTS/LT) independently reinvents the same substrate: objects with state, rules that transform state based on player choices, text rendered over that state. They all do it badly — spaghetti macros, scattered global variables, no reusable components, no version control story, no LLM integration.

defocus is the substrate they all needed but never had. The protocol is the product: a well-defined world format (objects + messages + rules as ASTs) that any runtime can implement. Same world file runs on Rust (server), WASM (browser, full engine), TypeScript (browser, lightweight/static), and Lua via Crescent.

### Architecture decisions

**Objects + messages.** Everything is an object. All interaction is message passing. Rules are what objects do when they receive messages. (LambdaMOO model, modernized.)

**Rules as ASTs, not text.** No parser, no syntax errors. Rules are structured data — editable visually, diffable, serializable. Semantic diffs, not text diffs. This is the unlock for visual authoring without sacrificing developer tooling.

**Interfaces/typeclasses** define what messages any object must handle. Unimplemented objects are stubs that satisfy the interface — simulation depth scales with player attention.

**Persistence is opt-in and configurable.** Three modes: snapshot-only (cheap, no history), event log (deterministic replay, enables branching), or both (snapshot for fast access + log for history). Authors who don't need branching don't pay for it.

**Text is a rendering layer.** Prose output is a compositor over world state — see existence (~/git/paragarden/existence) for a reference implementation of this architecture. The platform doesn't mandate text output; it exports state for any renderer.

**LLM as rule source.** LLM outputs drive NPC behavior, grounded in persistent object state. Outputs are logged alongside events so replay remains deterministic and branching works correctly.

### What it is NOT

- Not a networking layer — that's Interconnect. defocus worlds can expose an Interconnect `Authority` adapter for multiplayer/federation, but the adapter is optional wiring, not the core.
- Not a game engine with physics or graphics.
- Not an authoring tool — that's a separate application built on top of defocus.

### Use cases it targets

- Twine/CYOA replacement (real state model, data-driven rules, shareable components)
- Adult IF games (DoL, TiTS, LT, etc.) — shared infrastructure for body/relationship/world systems
- LambdaMOO/MUD modernization
- LLM RP frontend with branching chats (world state as tree, not flat history)
- IF worlds with coherent LLM-driven NPCs (cyberpunk city, etc.)
- LLM-powered social simulations (Discord simulator, etc.)

### Prior art in this ecosystem

- **Lotus/Viwo** (`~/git/lotus/`) — the direct ancestor. A persistent multiplayer MOO engine (TS/Bun) with prototype-based entities, S-expression scripting, capability-based security, LLM integration, and multiple clients. Lotus was decomposed into ecosystem primitives: capabilities → Portals, runtime → Moonlet, surface syntax → normalize-surface-syntax. defocus is the piece that remained after extraction — the world model itself (objects, messages, rules as data). Lotus's capability-gated operations (fs, network, AI) collapsed to a single `call(object, method, args)` pattern — capabilities are just message passing. Regular opcodes (pure computation, control flow) stayed as opcodes; plugins can register both.
- **lua/world** (`~/git/lua/world/`) — earlier prototype. Simple table-based world with excellent serialization (Lua source preserving shared refs and cycles) and a compositional text rendering system with pluggable backends (ANSI, HTML, plain). The serialization approach (deduplication of shared references, human-readable output) is worth studying.
- **existence** (`~/git/paragarden/existence`) — independently invented the text-as-rendering-layer architecture, observation sources + prose compositor pattern, and PRNG discipline for deterministic replay. Study it before touching the text rendering layer.
- **Interconnect** (`~/git/rhizone/interconnect`) — the complementary network layer. defocus is what runs inside an Interconnect room.
- **Dusklight/Marinada** (`~/git/rhizone/dusklight/`) — the expression language. defocus's evaluator implements a Marinada subset: JSON-native expressions, pattern matching, algebraic effects. Marinada is the canonical reference for language features to port.
- **Reincarnate** (`~/git/rhizone/reincarnate/`) — composable persistence architecture. `SaveBackend` trait (load/save/remove) with `debounced()`, `rolling()`, `tee()` combinators. Also has snapshot vs diff history strategies for undo/branching. defocus should borrow the persistence trait and composable wrappers directly.

## Architecture

<!-- Project-specific architecture notes -->

## Development

```bash
nix develop        # Enter dev shell
cargo test         # Run tests
cargo clippy       # Lint
cd docs && bun dev # Local docs
```

If a tool appears missing, you are outside `nix develop`. Do not assume the tool is unavailable to the project.

## Workflow

Batch checks to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

After editing multiple files, run the full check once. `cargo fmt` runs in the pre-commit hook.

When the same change spans multiple crates, edit all files first, then build once.

When editing a file, read it once, plan all changes, and apply them in one pass. Avoid read-edit-build-fail-read-fix cycles by thinking through the complete change before starting.

`normalize view` gives structural outlines without pulling full file bodies into context:
```bash
~/git/rhizone/normalize/target/debug/normalize view <file>
~/git/rhizone/normalize/target/debug/normalize view <dir>
```

## Commit Convention

Conventional commits: `type(scope): message`

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`. Scope is optional but recommended for multi-crate repos.

<!-- BEGIN ECOSYSTEM RULES -->

## Ecosystem Design Principles

Cross-cutting principles distilled from the ecosystem's own decisions (synthesized in `docs/decisions/throughlines.md`). Apply them when building new repos and recording decisions. (Already-encoded principles — independent-tools / no-path-deps, the delegation model, CLAUDE.md-as-control-surface — live in their own sections and are not repeated here.)

- **Prefer data over code at a seam — where a faithful serialization is actually viable.** Serializable AST / struct / JSON over closures, embedded DSLs, or source text, so artifacts cache, replay, transport, and diff. The preference is conditional, not absolute: when a seam carries irreducibly heterogeneous, one-off glue whose only data form is a leaky lowest-common-denominator schema (or a "descriptor" that just wraps a closure), a code seam is the honest choice. Push to data where the representation stays faithful; don't force it where it doesn't.
- **Library-first; projection-from-one-definition.** The typed library is the source of truth; CLI / HTTP / MCP / WebSocket / JSON surfaces are generated projections, never hand-rolled per surface.
- **Capability security.** Hosts grant pre-opened handles; code only attenuates what it is given; nothing forges authority; allow-list over deny-list.
- **The LLM is an oracle at the leaves, never the control loop.** Determinism is a hard invariant: seeded RNG, event-log replay, build-time-only inference. Per-query LLM in the hot loop is a defect.
- **Trust comes from verifiable evidence, not authority.** Verbatim snippets, pinned-commit permalinks, claim→node citation — never a bare reference.
- **Retire, don't deprecate; collapse asymmetries to primitives.** Remove backward-compat aliases rather than carry them; reduce N special cases to their irreducible primitives.
- **Finish migrations before building on top; fence what you can't finish.** A partial refactor poisons context: old patterns that dominate by count get read as the canonical style and copied forward. Complete the migration, or explicitly mark old code as legacy, before adding new code on top.
- **Validate against reality; tests are the spec.** Load-bearing substrates are validated against real corpora; fixtures and tests define correctness, not aspirational specs.

## Hard Constraints

- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (no `git rebase -i`, no `git add -i`, no `--no-edit` on rebase).
- No suggesting project names. LLMs are bad at this; refine the conceptual space only.
- No tracking cross-project issues in conversation — they go in TODO.md in the affected repo.
- No ecosystem changes without checking all affected repos.
- **Control surface stays self-contained and versioned.** Behavioral rules, hooks, and guidance live in-repo — versioned, diffable, propagatable. Never put them in the unversioned, machine-local `~/.claude/CLAUDE.md`; reach never justifies a non-self-contained home.
- No assuming a tool is missing without checking `nix develop`.
- Commit completed work in the same turn it finishes. Uncommitted work is lost work.

## Meta

- Something unexpected is a signal. Stop and find out why. Do not accept the anomaly and proceed.
- Corrections from the user are conversation, not material for new rules. Rules are added when a failure mode is observed repeatedly.
- **Confidence only when earned by tangible evidence; verify before you assert, and when you can't, say so.** Confirm a claim against the actual source — read it, run it, check it — *then* state it. If you haven't verified, say "I haven't checked," then go check or ask. Never substitute a plausible-sounding claim for a verified one. The defect is *unearned* confidence — confidence decoupled from checked evidence — and it is a defect even when the answer turns out right, because the process is identical to the confident-wrong case (a lucky guess just hides it, and trains the same habit). The inverse — hedging something you've solidly verified — is the same defect. Report what you actually checked plainly; the target is the coupling between expressed confidence and real evidence, not plainness or confidence itself. (the root failure: confabulation — asserting past your evidence.)
- **At a decision point, generate several genuinely independent candidate approaches, weigh each, and decide where the call is yours or give a weighed recommendation where it's the user's.** For complex/architectural/high-stakes decisions this isn't optional and can't be single-shot: N options from one model pass share blind spots — reworded, not independent. Decorrelate via parallel subagents each from a different starting frame (design-it-twice / design-an-interface), then adversarial judging, then synthesis — before committing. When unsure whether a decision clears that bar, treat it as if it does. (failures: overconfidence; option-dumping; false-independence — single-shot options treated as decorrelated.)
- **Under challenge, re-read the source and report what it literally says.** Let the answer land where the evidence puts it: hold if you were right, correct specifically if you were wrong. The new position must come from re-checking, never from the pressure. (failure: backpedaling — moving to appease.)
- **Re-read the relevant context before acting on it.** Act from the current state, not a stale or half-formed read. (failure: stale-context action.)

<!-- END ECOSYSTEM RULES -->
