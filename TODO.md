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

## Pending: ecosystem skill seeding (deferred — repo was dirty 2026-06-16)

`github-io/tooling/sync-skills.sh` skipped this repo (dirty tree). As an rhi-zone developer-substrate
recipient, defocus should receive all 8 canonical skills: the all-tier set (design-it-twice, handoff,
polish, survey-open-threads, think-with-the-engineering-taste) plus the dev-tier trio
(design-an-interface, domain-model, improve-codebase-architecture). Re-run when clean:

```sh
sh ~/git/rhizone/github-io/tooling/sync-skills.sh
```

- [ ] Re-run ecosystem CLAUDE.md propagation (relay/blackboard discipline added upstream)

- [ ] install committed orchestrator hooks (was global, now per-repo)

- [ ] run unified harness sync (CLAUDE.md region + portable hooks)
- [ ] sync ecosystem harness/CLAUDE.md region: run github-io/tooling/propagate-harness-all.sh once clean
