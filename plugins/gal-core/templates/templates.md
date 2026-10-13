# Templates

Prompt templates and file templates for the development workflow.
These are runtime-agnostic — any AI tool can consume them.

## File Templates

| Template | Output Path | Purpose |
| --- | --- | --- |
| [project.md](project.md) | `<repo>/.dev/project.md` | Portable project context |
| [state.md](state.md) | `<repo>/.dev/state.md` | Cross-session work state |
| [plan.md](plan.md) | `<repo>/.dev/plans/<plan-slug>.md` | Source plan doc (human-readable scope, rationale, requirements) consumed by `/planning`, `/deep-planning`, and optional planning providers that emit source plans. |
| [plan-prompt.md](plan-prompt.md) | `<repo>/.dev/plans/<plan-slug>.prompt.md` | Mutable execution work file derived from the source plan and consumed by `/gal status`, `/gal whats-next`, `/gal pipeline`, and specialist write-back flows. |
| [agent.md](agent.md) | `agents/*.agent.md` | Golem agent scaffold |

## Other Files

| File | Purpose |
| --- | --- |
| [csharp-convention.example.md](csharp-convention.example.md) | Inert example for a machine-local personal C# convention file |
| [structure-map.template.ndjson](structure-map.template.ndjson) | Seed template for `docs/structure/structure-map.ndjson` |
| [templates.md](templates.md) | This file — the template roster itself |

## Related Docs

- [workflows/coding.md](../workflows/coding.md) - plan lifecycle and workflow checkpoints
