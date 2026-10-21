# Conventions

Portable rules that ALL Golem agents follow, regardless of which tool is being used.

## Active Conventions

| File | Content |
| --- | --- |
| [rust.md](rust.md) | Rust: safety, naming (RFC 430), data models, thiserror/anyhow |
| [core-vs-personal.md](core-vs-personal.md) | First-class boundary between portable GAL Core and machine-local Personal Enhancement |
| [writing-quality.md](writing-quality.md) | Neutral writing contract: accuracy, complete meaning, protected spans, optional checks, and bounded repair |
| [self-bootstrap.md](self-bootstrap.md) | Self-bootstrap install lease, pinned hash, and pre-gate executable recheck |
| [handoff-notes.md](handoff-notes.md) | Canonical retry, interruption, human handback, and finalize handoff block formats |
| [naming.md](naming.md) | Project-wide naming authority (`docs/glossary.md`): reserved words, qualify overloaded terms, provenance discipline, identifier formation, naming gate |
| [task-atomicity.md](task-atomicity.md) | Canonical task-atomicity anti-patterns + atomicity principle + plan-size bound (≤99 tasks); the operational rubric lives in `/refining-plan` |
| [task-quality.md](task-quality.md) | Planner questions for task specification quality, consumed by `/refining-plan` and the pipeline task check |
| [minimalism.md](minimalism.md) | Minimalism ladder: the mandatory over-design check applied at the converge step of `/planning` and every adversarial review |
| [open-questions.md](open-questions.md) | Canonical OQ authority classes (H human / A architect / F false) + closure rule (B1–B7): refining never closes, architect-role closes A/F with recorded rationale, doubt→H |
| [token-budget.md](token-budget.md) | Token management: cold start priority, context handoff, knowledge flow |
| [optional-capabilities.md](optional-capabilities.md) | Five-state preflight for optional capabilities, including textlint and zhtw, structural-retrieval routing, and honest degradation |
| [working-hours.md](working-hours.md) | opt-in working-hours, After Hours, Wrap-up Time, and Hard Stop enforcement |

## Shared Skills

Universal cross-project guidance now lives in standalone skills:

- [git-commits](../skills/git-commits/SKILL.md)
- [structured-logging](../skills/structured-logging/SKILL.md)
- [result-pattern](../skills/result-pattern/SKILL.md)
- [markdown-formatting](../skills/markdown-formatting/SKILL.md)
- [adversarial-review](../skills/adversarial-review/SKILL.md)
