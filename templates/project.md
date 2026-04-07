# [Project Name]

## What This Is

[2-3 sentences: what does this product do and who is it for?]

## Tech Stack

| Layer | Technology |
| --- | --- |
| Language | [Language / framework / major libs / infrastructure] |
| Framework | |
| Database | |
| Testing | |
| CI/CD | |

## Architecture

[Brief description of architecture pattern: Clean Architecture, Vertical Slices, etc.]

### Layer Map

| Layer | Namespace | Depends On |
| --- | --- | --- |
| Domain | | Nothing |
| Application | | Domain |
| Infrastructure | | Application, Domain |
| Presentation | | Application |

## Constraints

- [Constraint 1] — [reason]

## Protected Paths

- [path/or/file] — [why touching this should escalate to T2]

## Key Decisions

| Decision | Rationale | Date |
| --- | --- | --- |
| [Choice] | [Why] | YYYY-MM-DD |

## Source Documents

[Index of canonical docs...]

| Document | Path | Last Verified |
| --- | --- | --- |

## Verified Facts

- [Tech stack: confirmed by inspecting .csproj / package.json]

## Suspected Drift

- [docs/api.md says endpoint X exists, but not found in codebase]

## Documentation Gaps

- [No ADR for database choice]
