# ADR-001: Pattern Matching Semantics

**Status:** Accepted  
**Date:** 2026-05-03

## Context

defocus needs pattern matching in its expression evaluator for payload dispatch — the primary use case is interactive narrative, where NPC handlers branch on the content of incoming messages:

```json
["match", ["get", "payload"],
  ["friendly", ["perform", "set", "mood", "happy"]],
  ["hostile",  ["perform", "set", "mood", "angry"]],
  ["_",        ["perform", "set", "mood", "confused"]]]
```

The reference language, Marinada, has a `match` form too, but it is **variant-only**: the first element of each arm is always a variant tag (always uppercase by convention), and remaining elements are always bindings. There is no literal string matching. This works because Marinada's values are algebraic — strings appear inside variants, not as bare payloads.

defocus worlds use bare strings (and other JSON primitives) as message payloads directly. Matching on them without wrapping in a variant is essential for ergonomics.

## Decision

`match_pattern` interprets patterns as follows:

| Pattern | Meaning |
|---------|---------|
| `"_"` | Wildcard — matches anything, binds nothing |
| `"$name"` | Binding — matches anything, binds scrutinee as `name` in scope |
| Any other string | Literal — matches only the identical string value |
| null / bool / int / float | Literal — matches only the identical value |
| Array | Structural — matches element-wise (same length required) |
| Record | Structural — matches if all keys in pattern are present and match in scrutinee |

The `$` prefix is the sigil that distinguishes bindings from literals. Without it, every string would need to be treated as either always-literal or always-binding — both are wrong for narrative use cases.

## Alternatives Considered

**Uppercase = literal, lowercase = binding (Marinada convention):** Works when you control the value space (algebraic types). Breaks for narrative payload matching where the strings are authored content — `"friendly"` should match the literal string `"friendly"`, not bind a variable named `friendly`.

**Variant wrapping (always use `["Tag", payload]` forms):** Correct but verbose. Requires every message payload to be a tagged variant rather than a plain string or number. Imposes unnecessary overhead on simple dispatch cases.

**Separate `literal` wrapper:** `["literal", "friendly"]` to distinguish from a binding `"friendly"`. More explicit but noisier. The `$` sigil achieves the same disambiguation with less syntax.

## Known Limitation

There is no way to match a literal string that begins with `$`. The pattern `"$foo"` is always interpreted as a binding, so the literal string `"$foo"` is unreachable in a match arm. In practice this is unlikely to matter (message payloads in narrative worlds don't start with `$`), but if it becomes necessary an escape convention (e.g. `"\\$foo"`) would be the fix.

## Relationship to Marinada

This design diverges from Marinada intentionally. Marinada's match is variant-only because it targets algebraic data; defocus's match is structural with literal support because it targets JSON world state and narrative payloads. The `$` binding prefix is defocus-specific and does not need to be back-ported to Marinada.
