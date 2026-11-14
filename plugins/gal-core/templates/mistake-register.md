# Mistake Register

Use this register to carry evidence-backed corrections between planning and closeout. Record observed behavior and its concrete evidence. State a cause only when the evidence verifies it. Do not create a row when there is no new, supported fact.

When adding an entry, include the correction and a verification indicator that can show whether the correction worked. Do not add the same event again when its evidence is already recorded. Add a distinct entry only when new evidence establishes a separate occurrence or failure mode.

Each correction uses schema version `1`. Record the correction ID and revision, source incident IDs, mechanism ID, path, level and reason, rule ID, version and effective time, verification status, and evidence references with their SHA-256 digests. Use null for an unknown rule version or effective time. An empty source incident list means the association is unknown.

## Recurring Mistakes

Use this table only when evidence establishes that the same mistake pattern recurred across tasks or sessions. A single observed failure does not establish recurrence.
In Correction, name the mechanism location and level, or state `judgment` and the reason.

| Observed symptom or pattern | Concrete evidence | Correction | Verification indicator |
| --- | --- | --- | --- |

## Subsystem Failure Modes

Use this table for a verified failure tied to a named subsystem, including a single established occurrence. Leave the cause unstated when it has not been established by evidence.

| Subsystem | Observed failure mode | Concrete evidence | Correction | Verification indicator |
| --- | --- | --- | --- | --- |
