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

## Core Rules

**docs/ reflects current architecture, not historical architecture.** When code changes, docs/ changes with it — in the same commit. New pages go in the sidebar immediately. The code and docs are one artifact.

**Note things down immediately — no deferral:**
- Problems, tech debt, issues → TODO.md now, in the same response
- Design decisions, key insights → docs/ or CLAUDE.md
- Future/deferred scope → TODO.md **before** writing any code, not after
- **Every observed problem → TODO.md. No exceptions.** Code comments and conversation mentions are not tracked items. If you write a TODO comment in source, the next action is to open TODO.md and write the entry.

**Conversation is not memory.** Anything said in chat evaporates at session end. If it implies a future behavior change, write it to CLAUDE.md immediately — or it will not happen.

**Warning — these phrases mean something needs to be written down right now:**
- "I won't do X again" / "I'll remember to..." / "I've learned that..."
- "Next time I'll..." / "From now on I'll..."
- Any acknowledgement of a recurring error without a corresponding CLAUDE.md edit

**Triggers:** User corrects you, 2+ failed attempts, "aha" moment, framework quirk discovered → document before proceeding.

**When the user corrects you:** Ask what rule would have prevented this, and write it before proceeding. **"The rule exists, I just didn't follow it" is never the diagnosis** — a rule that doesn't prevent the failure it describes is incomplete; fix the rule, not your behavior.

**Corrections are documentation lag, not model failure.** When the same mistake recurs, the fix is writing the invariant down — not repeating the correction. Every correction that doesn't produce a CLAUDE.md edit will happen again. Exception: during active design, corrections are the work itself — don't prematurely document a design that hasn't settled yet.

**Something unexpected is a signal, not noise.** Surprising output, anomalous numbers, files containing what they shouldn't — stop and ask why before continuing. Don't accept anomalies and move on.

**Do the work properly.** Don't leave workarounds or hacks undocumented. When asked to analyze X, actually read X — don't synthesize from conversation.

## Design Principles

**Unify, don't multiply.** One interface for multiple cases > separate interfaces. Plugin systems > hardcoded switches.

**Simplicity over cleverness.** HashMap > inventory crate. OnceLock > lazy_static. Functions > traits until you need the trait. Use ecosystem tooling over hand-rolling.

**Explicit over implicit.** Log when skipping. Show what's at stake before refusing.

**Separate niche from shared.** Don't bloat shared config with feature-specific data. Use separate files for specialized data.

## Workflow

**Batch cargo commands** to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test
```
After editing multiple files, run the full check once — not after each edit. Formatting is handled automatically by the pre-commit hook (`cargo fmt`).

**When making the same change across multiple crates**, edit all files first, then build once.

**Minimize file churn.** When editing a file, read it once, plan all changes, and apply them in one pass. Avoid read-edit-build-fail-read-fix cycles by thinking through the complete change before starting.

**`normalize view` is available** for structural outlines of files and directories:
```bash
~/git/rhizone/normalize/target/debug/normalize view <file>    # outline with line numbers
~/git/rhizone/normalize/target/debug/normalize view <dir>     # directory structure
```

**Always commit completed work.** After tests pass, commit immediately — don't wait to be asked. When a plan has multiple phases, commit after each phase passes. Do not accumulate changes across phases. Uncommitted work is lost work.

## Context Management

**Use subagents to protect the main context window.** For broad exploration or mechanical multi-file work, delegate to an Explore or general-purpose subagent rather than running searches inline. The subagent returns a distilled summary; raw tool output stays out of the main context.

Rules of thumb:
- Research tasks (investigating a question, surveying patterns) → subagent; don't pollute main context with exploratory noise
- Searching >5 files or running >3 rounds of grep/read → use a subagent
- Codebase-wide analysis (architecture, patterns, cross-file survey) → always subagent
- Mechanical work across many files (applying the same change everywhere) → parallel subagents
- Single targeted lookup (one file, one symbol) → inline is fine

## Commit Convention

Use conventional commits: `type(scope): message`

Types:
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code change that neither fixes a bug nor adds a feature
- `docs` - Documentation only
- `chore` - Maintenance (deps, CI, etc.)
- `test` - Adding or updating tests

Scope is optional but recommended for multi-crate repos.

## Negative Constraints

Do not:
- Use Claude Code's auto-memory system (`~/.claude/projects/.../memory/`) — it is unversioned, invisible to the user, and can't be diffed or backed up. Write behavioral changes and project context directly to CLAUDE.md instead
- Announce actions ("I will now...") - just do them
- Leave work uncommitted
- Use interactive git commands (`git add -p`, `git add -i`, `git rebase -i`) — these block on stdin and hang in non-interactive shells; stage files by name instead
- Use path dependencies in Cargo.toml - causes clippy to stash changes across repos
- Use `--no-verify` - fix the issue or fix the hook
- Assume tools are missing - check if `nix develop` is available for the right environment
