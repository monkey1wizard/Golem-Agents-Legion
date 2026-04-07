# Skills Migration

This document records how GAL's current `skills/` and `conventions/` layout was derived from an earlier personal skill collection.

It exists so future maintainers understand which content was preserved as a reusable skill, which content was normalized into conventions, and which content was intentionally dropped.

## Migration Outcome

| Outcome | Meaning |
| --- | --- |
| Moved to `skills/` | Remains an explicit reusable capability or workflow package |
| Extracted to `conventions/` | Better expressed as portable engineering rules than as a skill |
| Not migrated | Superseded by GAL workflow or otherwise not worth carrying forward |

## Skills Kept As Skills

These remain standalone skill packages under `skills/`:

- `defuddle`
- `doc-coauthoring`
- `json-canvas`
- `local-first-search`
- `mcp-builder`
- `obsidian-bases`
- `obsidian-cli`
- `obsidian-knowledge-management`
- `obsidian-markdown`
- `pdf`
- `skill-creator`
- `webapp-testing`
- `git-commits`
- `structured-logging`
- `result-pattern`
- `markdown-formatting`

These were kept because they are tool-like capabilities, domain-specific operating procedures, or reusable workflows that do not belong inside the GAL state machine itself.

The four skills added later were split back out of `conventions/universal.md` because they are independently discoverable behaviors, not a single monolithic convention block.

## Content Extracted Into Conventions

These were normalized into the `conventions/` directory:

- `blazor-development`
- `clean-architecture`
- `csharp-development`
- `go-development`
- `rust-development`
- `typescript-development`

These were extracted because GAL treats language and process rules as portable conventions rather than tool-triggered skills.

`git-commit`, `logging`, `markdownlint`, and `result-pattern` were initially consolidated into `conventions/universal.md`, but that design was later reversed. They now live as standalone skills again so each concern can be discovered and invoked independently.

## Content Intentionally Not Migrated

The following were intentionally not carried over as standalone skills:

- `plan-first-development`
- `testing-strategy`
- the older GSD workflow skill set

These were not migrated because GAL's own planner, tester, reviewer, verifier, and workflow definitions absorb the same responsibility more coherently.

## Maintenance Rule

When a skill starts acting like a stable cross-project rule, move that knowledge toward `conventions/`.
When a rule starts requiring procedural steps, examples, or dedicated tooling behavior, a standalone skill may still be justified.

## Related Docs

- [docs/design-principles.md](design-principles.md)
- [conventions/conventions.md](../conventions/conventions.md)
- [README.md](../README.md)
