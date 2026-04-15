# Templates

Prompt templates and file templates for the development workflow.
These are runtime-agnostic — any AI tool can consume them.

## File Templates

| Template | Output Path | Purpose |
| --- | --- | --- |
| [project.md](project.md) | `<repo>/.dev/project.md` | Portable project context |
| [state.md](state.md) | `<repo>/.dev/state.md` | Cross-session work state |
| [plan.md](plan.md) | `<repo>/docs/plans/<plan-slug>.md` | Source plan doc (human-readable scope, rationale, requirements) consumed by `/planning`, `/deep-planning`, and optional planning providers that emit source plans. |
| [plan-prompt.md](plan-prompt.md) | `<repo>/.dev/plans/<plan-slug>.prompt.md` | Mutable execution work file derived from the source plan and consumed by `/gal status`, `/gal whats-next`, `/gal pipeline`, and specialist write-back flows. |
| [diary.md](diary.md) | Obsidian vault | Scribe daily diary |
| [agent.md](agent.md) | `agent/*.agent.md` | Golem agent scaffold |

## Related Docs

- [docs/per-repo-context.md](../docs/per-repo-context.md) - why `.dev/` and plan files are split the way they are
- [workflows/coding.md](../workflows/coding.md) - plan lifecycle and workflow checkpoints
