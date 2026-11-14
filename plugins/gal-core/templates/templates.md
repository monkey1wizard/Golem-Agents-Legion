# Templates

Prompt templates and file templates for the development workflow.
These are runtime-agnostic — any AI tool can consume them.

## File Templates

| Template | Output Path | Purpose | Diátaxis Mode | Writer | Gate |
| --- | --- | --- | --- | --- | --- |
| [project.md](project.md) | `<repo>/.dev/project.md` | Portable project context | Reference | Any producer | `gal init` reads this template |
| [state.md](state.md) | `<repo>/.dev/state.md` | Cross-session work state | Reference | Any producer | `gal state-merge` consumes this section shape |
| [plan.md](plan.md) | `<repo>/.dev/plans/<plan-slug>.md` | Source plan with scope, rationale, and requirements | Five zones, one mode each | Any producer | `gal planning-check` and `gal refining-check` |
| [plan-prompt.md](plan-prompt.md) | `<repo>/.dev/plans/<plan-slug>.prompt.md` | Mutable execution work file derived from the source plan | How-to | Any producer | `gal prompt-check` |
| [mistake-register.md](mistake-register.md) | `<repo>/.dev/mistake-register.md` | Evidence-backed recurring mistakes and subsystem failure modes | Reference | Wrap-up and finalize | Read before refining; updated from verified evidence |
| [agent.md](agent.md) | `agents/*.agent.md` | Golem agent scaffold | How-to | Any producer | Agent author review |

## Other Files

| File | Purpose | Diátaxis Mode | Writer | Gate |
| --- | --- | --- | --- | --- |
| [csharp-convention.example.md](csharp-convention.example.md) | Inert example for a machine-local personal C# convention file | Reference | Any producer | Convention consumer |
| [structure-map.template.ndjson](structure-map.template.ndjson) | Seed template for `docs/structure/structure-map.ndjson` | Reference | Any producer | Structure-map tooling |
| [templates.md](templates.md) | This file — the template roster itself | Reference | Any producer | `finalize_hygiene` checks file inventory |

## Related Docs

- [workflows/coding.md](../workflows/coding.md) - plan lifecycle and workflow checkpoints
