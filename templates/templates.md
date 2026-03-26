# Templates

Prompt templates and file templates for the development workflow.
These are runtime-agnostic — any AI tool can consume them.

## File Templates

| Template | Output Path | Purpose |
|----------|------------|--------|
| [project.md](project.md) | `<repo>/.dev/project.md` | Portable project context |
| [state.md](state.md) | `<repo>/.dev/state.md` | Cross-session work state |
| [plan.md](plan.md) | `<repo>/docs/plans/*.prompt.md` | Plan scaffold for `gal plan` |
| [diary.md](diary.md) | Obsidian vault | Scribe daily diary |
| [agent.md](agent.md) | `agent/*.agent.md` | Golem agent scaffold |
