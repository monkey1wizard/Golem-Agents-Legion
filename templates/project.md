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

### Godot Example

If this is a Godot C# repo, record the split explicitly:

| Layer | Technology |
| --- | --- |
| Language | C# (.NET 8 for Godot runtime code, newer .NET only for external tooling) |
| Framework | Godot 4.x |
| Database | N/A or project-specific |
| Testing | `godot --headless --build-solutions`, `dotnet test`, runtime smoke tests |
| CI/CD | Headless Godot export plus the repo's CI runner |

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

- [path/or/file] — [why touching this should force a return to /deep-planning before implementation continues]

## Key Decisions

| Decision | Rationale | Date |
| --- | --- | --- |
| [Choice] | [Why] | YYYY-MM-DD |

## Source Documents

[Index of source docs...]

| Document | Path | Last Verified |
| --- | --- | --- |

## Verified Facts

- [Tech stack: confirmed by inspecting .csproj / package.json]

## Suspected Drift

- [docs/api.md says endpoint X exists, but not found in codebase]

## Documentation Gaps

- [No ADR for database choice]
