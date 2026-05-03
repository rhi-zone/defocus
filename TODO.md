# TODO

## Open

### [ ] match: no way to match a literal string starting with `$`

`match_pattern` treats any string beginning with `$` as a binding variable. There is currently no escape sequence to match the literal string `"$foo"`. Unlikely to matter in practice (narrative payloads don't start with `$`), but if needed, add an escape convention such as `"\\$foo"`. See ADR-001.
