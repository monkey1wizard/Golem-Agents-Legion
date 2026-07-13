# Conventions

Portable rules that ALL Golem agents follow, regardless of which tool is being used.

## Active Conventions

| File | Content |
| --- | --- |
| [rust.md](rust.md) | Rust: safety, naming (RFC 430), data models, thiserror/anyhow |
| [core-vs-personal.md](core-vs-personal.md) | First-class boundary between portable GAL Core and machine-local Personal Enhancement |
| [naming.md](naming.md) | Project-wide naming authority (`docs/naming.md`): reserved words, qualify overloaded terms, provenance discipline, identifier formation, naming gate |
| [task-atomicity.md](task-atomicity.md) | Canonical task-atomicity anti-patterns + atomicity principle + plan-size bound (≤99 tasks); the operational rubric lives in `/refining-plan` |
| [minimalism.md](minimalism.md) | Minimalism ladder: the mandatory over-design check applied at the converge step of `/planning` and every adversarial review |
| [open-questions.md](open-questions.md) | Canonical OQ authority classes (H human / A architect / F false) + closure rule (B1–B7): refining never closes, architect-role closes A/F with recorded rationale, doubt→H |
| [token-budget.md](token-budget.md) | Token management: cold start priority, context handoff, knowledge flow |
| [working-hours.md](working-hours.md) | opt-in working-hours, After Hours, Wrap-up Time, and Hard Stop enforcement |

## Shared Skills

Universal cross-project guidance now lives in standalone skills:

- [git-commits](../skills/git-commits/SKILL.md)
- [structured-logging](../skills/structured-logging/SKILL.md)
- [result-pattern](../skills/result-pattern/SKILL.md)
- [markdown-formatting](../skills/markdown-formatting/SKILL.md)
- [adversarial-review](../skills/adversarial-review/SKILL.md)
