# TODO

## Open

### [ ] match: no way to match a literal string starting with `$`

`match_pattern` treats any string beginning with `$` as a binding variable. There is currently no escape sequence to match the literal string `"$foo"`. Unlikely to matter in practice (narrative payloads don't start with `$`), but if needed, add an escape convention such as `"\\$foo"`. See ADR-001.


## Pending: ecosystem-rules region sync (deferred — repo was dirty 2026-06-15)

The canonical ecosystem-rules region (github-io commit e678388) dropped two harness-management bullets
("No ecosystem changes without checking all affected repos." and "Control surface stays self-contained and versioned.").
This region sync was deferred because this repo had uncommitted work. Re-run when clean:

```sh
sh ~/git/rhizone/github-io/tooling/propagate-claude-md.sh "$(git rev-parse --show-toplevel)/CLAUDE.md"
git add CLAUDE.md
git commit -m "docs(claude): sync ecosystem rules — drop harness-management bullets"
git push
```
