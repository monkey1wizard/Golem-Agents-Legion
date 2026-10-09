# Handoff Notes

Every heading in this file stays byte-identical. Consumers reference this file pointer-only. The `gal` binary parses the `Retry Handoff — ` and `Human Handback — ` prefixes byte-exact, including the em dash, its surrounding spaces, and the trailing space after `Human Handback —`. No code parses the `Interrupted Phase` headings; preflight reads the `Interrupted Phase:` state line instead.

## Retry And Blocker Handoff Contract

When a task hits repeated failure or an immediate human-required stop, append or refresh a single task-scoped block in `### Handoff Notes` using this format:

```markdown
#### Retry Handoff — T-NN / [IMPLEMENT | TEST | AUDIT]

- Status: OPEN | RESOLVED
- Problem: <latest blocking problem statement>
- Evidence:
  - Test Results: <latest task-scoped subsection or `not-applicable`>
  - Review Results: <latest task-scoped subsection or `not-applicable`>
  - Security Review: <latest task-scoped subsection or `not-applicable`>
- Attempts:
  1. <YYYY-MM-DD> — <attempt summary>
     - Result: <what changed or why it still failed>
     - Validation: <command, auditor verdict, or `not-run`>
     - Commit: <hash or `none`>
  2. ...
- Next human step: <exact next inspection or repair step>
```

Rules:

- Keep exactly one `OPEN` handoff block per `Current Task` and active phase. Update the existing block instead of appending duplicates.
- Record every retry-triggered fix attempt in order. By the third failed `TEST` or `AUDIT` round, the handoff must tell the human what was tried on attempts 1-3 without reconstructing history from chat.
- When a later rerun clears the issue, keep the block for history but change `Status` to `RESOLVED` and replace `Next human step` with the confirmation that cleared it.

## Interruption Handoff Contract

Use interruption handoff for non-decision runtime cutoffs such as OpenCode `steps` exhaustion, provider max-turn limits, context exhaustion, or terminal/tool availability ending a phase before its convergence gate is reached.

If the current invocation receives a runtime message equivalent to "maximum steps reached" or resumes and finds an incomplete current phase, write or refresh this block before doing any unrelated work:

```markdown
#### Interrupted Phase — T-NN / [IMPLEMENT | TEST | REVIEW | SECURITY | VERIFY]

- Status: OPEN | RESOLVED
- Cause: runtime-step-limit | context-limit | tool-unavailable | unknown
- Workflow at interruption: <Workflow value>
- Durable state present:
  - Task Base Commit: <hash or `missing`>
  - Task Final Commit: <hash or `missing`>
  - Worktree: clean | dirty | unknown
  - Test Results: <task-scoped subsection present? yes/no/not-applicable>
  - Review Results: <task-scoped subsection plus verdict present? yes/no/not-applicable>
- Resume action: <exact next command or phase action>
```

Rules:

- Keep exactly one `OPEN` interrupted-phase block per task and phase. Update it instead of appending duplicates.
- An interrupted phase is not the same as a failed phase. Do not increment `Test Retry Count` or `Review Retry Count` unless a real failing test or review finding exists.
- On resume, clear the interrupted-phase block only after the phase's normal convergence gate succeeds.
- If the interrupted phase is `IMPLEMENT` and the worktree is dirty, continue from the existing changes, run the scoped verification, commit, and record `Task Final Commit`. Do not start a new task.

## Human Handback Contract

Every `human-required` result requires exactly one `OPEN` block under `### Handoff Notes` headed `#### Human Handback — <reason>` with these fixed fields:

```markdown
#### Human Handback — <reason>

- Status: OPEN | RESOLVED
- Reason: <reason>
- Task: <task identifier or `none`>
- Phase: IMPLEMENT | TEST | AUDIT | VERIFY | BOUNDARY | CONVERGE
- Producer: <producer>
- Producer state: <producer state>
- Git HEAD: <hash>
- Next human step: <exact next inspection or repair step>
- What to check: <the concrete artifact, command, or behavior the owner inspects>
- Expected result: <the observable outcome that means the check succeeded>
- Pass/fail rule: <the single rule that decides pass or fail from the observed outcome>
```

For `head-drift`, also include:

```markdown
- Baseline HEAD: <hash>
- Observed HEAD: <hash>
```

Rules:

- Keep exactly one `OPEN` block for each human-required result. The checker ignores historical `RESOLVED` blocks when enforcing this single-OPEN rule.
- When the issue clears, mark that block `RESOLVED` rather than creating a second live block.

## Finalize Interrupted Phase Contract

finalize is interruptible and resumable without repeating destructive steps. It uses this interrupted-phase marker shape in the execution prompt `## Status > ### Handoff Notes`:

```markdown
#### Interrupted Phase — finalize / [ACCEPTANCE | REVIEW | RELEASE | MERGE | TEARDOWN | CLOSE]

- Status: OPEN | RESOLVED
- Mode: worktree-branch | already-on-main
- Merge state: not-started | merged | not-applicable
- Resume action: <exact next step>
```

Resume rules:

- Every finalize stop leaves one `OPEN` marker, and its `Resume action` is `/gal finalize`. Never resume by restarting the ordinary pipeline on a `DONE` prompt.
- After a successful merge, set `Merge state: merged`. On resume with `Merge state: merged`, never re-merge; continue only teardown / close.
- Because plan-file deletion is the final step, the marker lives in the prompt until close completes; a resume always has a marker to read.
- Clear the marker (`Status: RESOLVED`) only when the phase's own completion condition is met.
- On resume, run the Owner Acceptance check first, then refresh every applicable finalize gate.

### ACCEPTANCE marker

Use the `ACCEPTANCE` phase when finalize finds `## Owner Acceptance` rows that have no valid evidence. Preserve the current `Mode` and `Merge state`. Add these fields to the marker:

```markdown
- Outstanding rows: <the exact OA-NN rows, shown verbatim from the table>
- What to check: <the concrete artifact, command, or behavior the owner inspects>
- Expected result: <the observable outcome that means the check succeeded>
- Pass/fail rule: <the single rule that decides pass or fail from the observed outcome>
- Resume command: /gal finalize
```

Rules:

- Writing this marker and the evidence rows below is the only work allowed while the stop is open.
- After the owner responds, resolve only the satisfied `ACCEPTANCE` interruption. The orchestrator commits only the required handoff and evidence state before the full-mode cleanliness gate. Unrelated dirty output is preserved, and finalize stops for its owner. It is never deleted or stashed.
- A material change to an accepted criterion or target reopens the affected rows only, with a new `ACCEPTANCE` marker. Unrelated metadata or documentation changes do not.

## Owner Acceptance Evidence Contract

The `## Owner Acceptance` table and its five columns do not change. Owner answers are recorded under `### Handoff Notes` in one table headed `#### Owner Acceptance Evidence`:

```markdown
#### Owner Acceptance Evidence

| ID | Outcome | Owner evidence | Target binding |
| --- | --- | --- | --- |
| OA-NN | accepted | <owner statement, date, message reference when available> | <criterion, and the result or artifact the owner evaluated> |
```

Rules:

- `ID` is an existing `OA-NN` from the `## Owner Acceptance` table.
- `Outcome` is `accepted` or `waived`. A waiver is recorded as a waiver and is never reported as a measured PASS.
- `Owner evidence` records the owner's actual statement and the date. It includes a message reference only when one is available, and never an invented identifier.
- `Target binding` names the criterion and the concrete result or artifact the owner evaluated, so a later change to that target can be detected.
- A row without valid evidence is outstanding. Pipeline startup, a bare finalize request, and a resume instruction are not evidence.
