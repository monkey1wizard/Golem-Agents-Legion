# Project Context Template

Template for `<repo>/.dev/project.md` — portable project context.

## File Template

```markdown
# [Project Name]

## What This Is

[2-3 sentences: what does this product do and who is it for?]

## Tech Stack

| Layer | Technology |
| --- | --- |
| Language | [e.g., C# 14 / .NET 10] |
| Framework | [e.g., .NET MAUI] |
| Database | [e.g., SQLite + Cloud Firestore] |
| Testing | [e.g., xUnit + FluentAssertions + NSubstitute] |
| CI/CD | [e.g., GitHub Actions] |

## Architecture

[Brief description of architecture pattern: Clean Architecture, Vertical Slices, etc.]

### Layer Map

| Layer | Namespace | Depends On |
| --- | --- | --- |
| Domain | Project.Domain | Nothing |
| Application | Project.Application | Domain |
| Infrastructure | Project.Infrastructure | Application, Domain |
| Presentation | Project.Presentation | Application |

## Active Skills

[List exact folder names from `golem-agents-legion/skills/` that apply to this project.
Put one skill per bullet. `gal sync` fails if this section is missing, empty, duplicated, or references a skill folder that does not exist.]

## Constraints

- [Constraint 1] — [reason]
- [Constraint 2] — [reason]

## Protected Paths

- [path/or/file] — [why touching this should escalate to T2]

## Key Decisions

| Decision | Rationale | Date |
| --- | --- | --- |
| [Choice] | [Why] | YYYY-MM-DD |

## Source Documents

[Index of canonical docs in this repo. Agents read project.md first; only follow these links when the summary is insufficient.]

| Document | Path | Last Verified |
| --- | --- | --- |
| [README] | [README.md] | [YYYY-MM-DD] |
| [Architecture] | [docs/architecture.md] | [YYYY-MM-DD] |

## Verified Facts

[Facts confirmed by code inspection — not just what docs claim.]

- [Tech stack: confirmed by inspecting .csproj / package.json]
- [Test framework: confirmed by running `dotnet test`]

## Suspected Drift

[Places where docs and code may disagree. Record rather than silently assume correct.]

- [docs/api.md says endpoint X exists, but not found in codebase]

## Documentation Gaps

[Known areas with no documentation. Prioritize creating these.]

- [No ADR for database choice]
- [No docs on deployment process]
```

## Usage Rules

1. **One per repo** — lives at `<repo>/.dev/project.md`
2. **Portable** — no tool-specific syntax, any AI can read it
3. **Concise** — context window is precious, don't bloat
4. **Active Skills** — use exact skill folder names; Sync-DevContext validates this list and fails hard when it is missing or stale
5. **Update on architecture changes** — not after every commit
6. **Summary + index, not duplication** — point to canonical docs, don't copy their content
7. **Record drift and gaps** — better to document uncertainty than ignore it
