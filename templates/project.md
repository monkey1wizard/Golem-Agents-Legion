# Project Context Template

Template for `<repo>/.dev/project.md` — portable project context.

## File Template

```markdown
# [Project Name]

## What This Is

[2-3 sentences: what does this product do and who is it for?]

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | [e.g., C# 14 / .NET 10] |
| Framework | [e.g., .NET MAUI] |
| Database | [e.g., SQLite + Cloud Firestore] |
| Testing | [e.g., xUnit + FluentAssertions + NSubstitute] |
| CI/CD | [e.g., GitHub Actions] |

## Architecture

[Brief description of architecture pattern: Clean Architecture, Vertical Slices, etc.]

### Layer Map

| Layer | Namespace | Depends On |
|-------|-----------|-----------|
| Domain | Project.Domain | Nothing |
| Application | Project.Application | Domain |
| Infrastructure | Project.Infrastructure | Application, Domain |
| Presentation | Project.Presentation | Application |

## Active Skills

[List skills from golem-agents-legion that apply to this project.
Sync-DevContext reads this field to inline skill content into adapters.]

- csharp-development
- clean-architecture
- testing-strategy

## Constraints

- [Constraint 1] — [reason]
- [Constraint 2] — [reason]

## Protected Paths

- [path/or/file] — [why touching this should escalate to T2]

## Key Decisions

| Decision | Rationale | Date |
|----------|-----------|------|
| [Choice] | [Why] | YYYY-MM-DD |
```

## Usage Rules

1. **One per repo** — lives at `<repo>/.dev/project.md`
2. **Portable** — no tool-specific syntax, any AI can read it
3. **Concise** — context window is precious, don't bloat
4. **Active Skills** — used by Sync-DevContext to generate adapters
5. **Update on architecture changes** — not after every commit
