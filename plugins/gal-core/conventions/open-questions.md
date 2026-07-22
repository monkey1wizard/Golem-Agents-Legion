# Open Questions — Authority Classes & Closure Rule

Canonical rule for how `## Open Questions` (`OQ-NN`) entries are classified and closed across the planning flow. `workflows/coding.md`, `commands/deep-planning`, `commands/refining-plan`, `templates/plan.md`, and `agents/golem-architect.agent.md` reference this file; they do not restate the rule.

## Why

The OQ-completion gate must not let a model smuggle an unreviewed owner decision past it. The failure mode: a model invents a "default" and closes a question that actually needed the human, during refining. This rule binds closure authority to the **class** of the question and to the **architect role** — not to whichever command happens to be running.

## Authority Classes

Every OQ has one authority class. The axis is *what authority the answer requires*, not "decision vs non-decision".

| Class | Definition | Who may close |
| --- | --- | --- |
| **H — human authority** | No engineering-correct answer; the answer depends on the owner's preference, risk appetite, priority, or an external fact the model cannot derive. Examples: taste / product / UX, business value, trust or security boundary acceptance, irreversible or costly commitment, scope ownership ("is this in scope"). | **Human only.** Architect/AI may *recommend*, never close. An override-able default the human has not accepted is **not** a closure (the OQ stays open). |
| **A — architect authority** | A genuine technical trade-off with an engineering-determinable answer that a competent architect can defend from repo + conventions + trade-offs alone. Examples: pattern/abstraction choice, module/file layout, dependency direction, over-engineering calls, naming *form* (RFC-430 / house-style). | **Architect role**, with recorded adversarial rationale (option chosen + trade-off named). Human override stays possible but is **not required** to proceed. |
| **F — false OQ** | Not actually a decision — inspection forces a single answer. | Architect role / evidence, with the proof there was no second defensible option. |

**Risk note:** the *analysis* of a risk is Class A (architect computes it); *accepting* the risk is Class H (owner authorizes).

## Boundaries (B1–B7)

- **B1 — H/A decision test.** Ask: *can technical reasoning over repo + conventions settle it without owner preference or a model-unreachable external fact?* Yes → A. No → H. Collapses to one answer → F. **Mixed** (technical core + owner-preference shell): if the residual preference would materially change the outcome → H; if cosmetic and the technical answer dominates → A. **Tie / genuine doubt → H.**
- **B2 — A/F boundary.** F requires proof of no second defensible option; if any real alternative survives, it is not F (it is A, or H).
- **B3 — role, not command.** Closure authority is bound to the role: A/F → architect role; H → human. The architect role closes A/F via **either** `/deep-planning` (which *guarantees* architect activation) **or** a direct `/gal architect` review that engages its formal write-back. A pure advice-only consult (no write-back engaged) does **not** close OQs. `/refining-plan`, `/planning`, and every other role have **zero** closure authority. The human may close H at any time, including interactively.
- **B4 — per-class authority.** The same "override-able architect default" is a **valid** closure for Class A and an **invalid** closure for Class H. This is the precise line the prior blanket gate clause blurred.
- **B5 — reclassification (asymmetric).** Escalating **A → H** is always allowed (anyone, any doubt). Demoting **H → A** (which unlocks model closure) requires architect to give an explicit technical justification that no preference or external fact is load-bearing; if that is not defensible, or in doubt, the OQ stays **H**.
- **B6 — record.** Closing an OQ deletes its entry (per the internalization rule — fold the decision into the plan body, no `[x] resolved by …` breadcrumbs), but the closure **must** leave the closer-class + rationale/evidence in `## Review Results` / git history. A "closed" OQ with no recorded basis is an **invalid** closure an auditor can reject.
- **B7 — skip-path.** Any open **A/F** OQ ⇒ the plan needs one architect closure (`/deep-planning` or a direct `/gal architect` review) before `/refining-plan`. Pure-**H** OQs may be human-closed interactively, then proceed. This is **orthogonal** to "a structural / Protected-Path change forces `/deep-planning`" — that fence is not waived just because an OQ was closed directly.

## Stage Invariants

- **`/planning`** — raises OQs; does not close.
- **`/deep-planning`** — the architect-activation closing path: architect classifies and closes A/F (with recorded rationale); H is surfaced + recommended to the human; **doubt → H**.
- **`/gal architect`** (direct, with write-back) — the same architect-role closing authority for A/F.
- **`/refining-plan`** — **zero closure authority.** It only gate-checks that all OQs are already resolved; any open OQ → refuse + return to `/deep-planning`. If refining is ever tempted to "resolve" an OQ to proceed, *that is the bug this convention exists to stop.*

## Class Tag

OQ entries carry a class tag: `- [ ] OQ-NN [H|A|F] — description *(raised by: command)*`. **An untagged OQ defaults to H** (the safest failure direction — over-strict, and an architect may escalate-to-A's-inverse only via the B5 justification).

## ID Format

`OQ-NN` is a **zero-padded two-digit number** (`OQ-01` … `OQ-99`). Three-digit form `OQ-NNN` is **never valid**. A plan may have at most 99 open questions; if more arise, the plan is too large and should be split.
