<!--
  .dev/project.md is a COMPRESSED INDEX of the durable documentation layer (README.md + docs/),
  not a knowledge sink. New verified knowledge goes into docs/ FIRST (or README.md for repo-level
  context), then is re-indexed here. Do not write durable facts directly into this file.
  Policy: docs/architecture.md#documentation-governance
-->

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
| Personal Conventions | off |

The `Language` row drives conventions selection at `gal init`: it selects gal-core's neutral
conventions, any matching personal convention file at `~/.gal/local/conventions/<lang>.md`, and any
installed agent-plugin skill whose name/origin matches (rendered as a Detected Language Skills
reference block). See `docs/configuration.md` for the full personal-coding-style guide.

The `| Personal Conventions | off |` row in this table disables the personal-convention-file and
detected-skill sources for this repo (recommended for public repos, so tracked adapters never embed
owner-machine content). Remove this row to enable personal conventions for this repo.

<!-- gal:authoritative-check -->
```json
{
  "command": [
    "[Command / runner / test command to run for authoritative verification]"
  ]
}
```

### Example

| Layer | Technology |
| --- | --- |
| Language | TypeScript, Node.js |
| Framework | Express |
| Database | PostgreSQL |
| Testing | `npm test` (Jest) |
| CI/CD | GitHub Actions |

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

## Response Style

- [How agents should shape replies in this repo, e.g. "default to maximum compression" or "explain reasoning in full sentences"]

## Freshness

- Always get today's date first, then use that date when querying for the latest information or other time-sensitive context.

## Project Language

- `PROJECT_LANGUAGE`: [BCP-47 tag for the canonical documentation language, e.g. `en`, `zh-TW`]
- Canonical docs use the main filename with no language infix (for example `README.md`).
- Translation copies use `<name>.<lang>.md` (for example `README.zh-Hant.md`).

## Protected Paths

- [path/or/file] — [why touching this should require a recorded architect review (APPROVE / `<!-- ARCH_REVIEW: CLEAR -->`) before implementation continues; obtained via /deep-planning or a direct /gal architect write-back]

## Key Decisions

| Decision | Rationale | Date |
| --- | --- | --- |
| [Choice] | [Why] | YYYY-MM-DD |

## Source Documents

[Index of source docs...]

| Document | Path | Last Verified |
| --- | --- | --- |

## Verified Facts

One bullet per fixed topic area, stating its **current** state only, with a durable pointer into `README.md` or `docs/`. Upsert/replace/prune — never append a new bullet to record that a plan finished; see `plugins/gal-core/conventions/token-budget.md#bounded-current-topic-index-devproject-md`.

### Example

- [Runtime & layout: current tech-stack summary and package/module layout — pointer: `README.md`, `docs/architecture.md`]

## Suspected Drift

None.

## Documentation Gaps

None.
