# Conventions

Portable rules that ALL Golem agents follow, regardless of which tool is being used.

## Active Conventions

| File | Content |
| --- | --- |
| [csharp.md](csharp.md) | C# / .NET 10 / C# 14: naming, modern syntax, Clean Architecture, Blazor |
| [go.md](go.md) | Go: error handling, naming, project structure, table-driven tests |
| [typescript.md](typescript.md) | TypeScript: strict typing, naming, async, testing |
| [rust.md](rust.md) | Rust: safety, naming (RFC 430), data models, thiserror/anyhow |
| [token-budget.md](token-budget.md) | Token management: cold start priority, context handoff, knowledge flow |
| [curfew.md](curfew.md) | 22:00 soft curfew + 23:00 hard curfew — shutdown enforcement |

## Shared Skills

Universal cross-project guidance now lives in standalone skills:

- [git-commits](../skills/git-commits/SKILL.md)
- [structured-logging](../skills/structured-logging/SKILL.md)
- [result-pattern](../skills/result-pattern/SKILL.md)
- [markdown-formatting](../skills/markdown-formatting/SKILL.md)
