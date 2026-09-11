# Research Flow

A workflow for research-driven tasks: investigating technologies, checking evidence quality, and writing durable results with repo-owned outputs first and optional external-note routing only when explicitly applicable.

`golem-researcher` owns the RESEARCH, SYNTHESIZE, and CROSS-REVIEW states. Each of these states runs inside three parallel isolated researcher workers, `RESEARCHER#0`, `RESEARCHER#1`, and `RESEARCHER#2`, each blind to the other two — no worker reads another worker's brief, findings, or synthesis before ORCHESTRATOR compares them in VERIFY. `RESEARCHER#0` is always a native subagent of the orchestrator's own provider and has no config key: it is never listed in `executorRouting.research` and never routed to a headless executor. `RESEARCHER#1` and `RESEARCHER#2` resolve independently, in this order: an explicit `research."1"` or `research."2"` entry in `executorRouting`, then `executors.agy` when defined, then a native subagent. Zero config resolves all three workers to native subagents. Reference verification's independence comes from worker isolation, not from a model-swap rule; routing it to an independent model that did not author the research findings is a strengthening measure on top of that guarantee, not an obligation. Any optional external-note write is routed through a machine-local note backend rather than a public vault-writer role.

This workflow is **independent of the Coding Flow**. It can run in parallel with active development, or as a standalone investigation.

## Modes

| Mode | When | States | Minimum source rule | Verification rule |
| --- | --- | --- | --- | --- |
| `research` | Standard structured investigation | RESEARCH (×3 parallel workers) → VERIFY → DOCUMENT | Each worker gathers enough sources to answer the question responsibly | Every cited reference must be reverse-checked (worker isolation is the independence guarantee; an independent model strengthens it) |
| `deep-research` | Higher-risk, higher-ambiguity, or cross-cutting investigation | RESEARCH → SYNTHESIZE → CROSS-REVIEW (×3 parallel workers, blind) → VERIFY → DOCUMENT | Each worker attempts at least 5 sources | Every cited reference must be reverse-checked (worker isolation is the independence guarantee; an independent model strengthens it) |

## Mandatory Rules

1. **Local-first remains mandatory**. Check codebase, repo docs, and repo-owned state before external sources; external notes are optional add-on evidence only.
2. **Every cited reference must exist**. Missing, dead, or unverifiable references must be removed or explicitly marked `[UNVERIFIED]` before documentation.
3. **Reference verification's independence comes from worker isolation, not from a model-swap rule**. The three research workers never read each other's brief, findings, or synthesis during RESEARCH, SYNTHESIZE, or CROSS-REVIEW, and that isolation is what keeps VERIFY's citation check independent of any single worker's conclusions. Routing VERIFY to a different executor or model than the research workers used is a strengthening measure on top of that guarantee, not an obligation. Honest limitation: ORCHESTRATOR itself is not blind. It reads and compares all three workers' briefs during VERIFY, so the guarantee covers worker-to-worker isolation, not orchestrator-to-worker blindness.
4. **`deep-research` must attempt at least 5 sources**. If fewer than 5 are available, document the failed search attempts and why the source pool is constrained.
5. **CROSS-REVIEW is about source-to-source consistency**. It is not a substitute for architecture, business, or design review.
6. **Playwright MCP is a dynamic-page aid, not a research default**. Use it only after local-first and structured retrieval paths cannot answer the question.
7. **Browser-backed research must preserve reverse-checkable evidence**. Record the URL, the interaction performed, and the artifact or observation that supports the claim.
8. **Research narrative follows one resolved output language**. Resolve it by explicit directive, machine-local `planLanguage` (`config.json`), prompt-language auto-detect, then fallback `en`.
9. **Keep the report fully in the resolved language**. Do not mix prose languages inside one research document, except for literal technical identifiers and preserved source material.
10. **Preserve citations in their original form**. Source titles, URLs, quoted snippets, and other directly cited original-language material stay unchanged even when the report narrative is translated.

## State Machine

Every RESEARCH, SYNTHESIZE, and CROSS-REVIEW state below runs three times in parallel, once inside each of `RESEARCHER#0`, `RESEARCHER#1`, and `RESEARCHER#2`. The three workers are isolated from each other for the full duration of these states — none of them can read another worker's brief, findings, or synthesis. ORCHESTRATOR is the first place the three outputs meet, and that meeting happens in VERIFY.

### `research`

```text
IDLE → RESEARCH#0 ─┐
       RESEARCH#1 ─┼→ VERIFY (ORCHESTRATOR compares 3 briefs) → DOCUMENT → DONE
       RESEARCH#2 ─┘
```

### `deep-research`

```text
IDLE → RESEARCH#0 ─┐          SYNTHESIZE#0 ─┐          CROSS-REVIEW#0 ─┐
       RESEARCH#1 ─┼→ (each) → SYNTHESIZE#1 ─┼→ (each) → CROSS-REVIEW#1 ─┼→ VERIFY (ORCHESTRATOR compares 3 briefs) → DOCUMENT → DONE
       RESEARCH#2 ─┘          SYNTHESIZE#2 ─┘          CROSS-REVIEW#2 ─┘
         ↑                       ↑                        ↑                  │
         └── evidence ───────────┴── synthesis ────────────┴── source/ref ───┘
```

Each worker advances RESEARCH → SYNTHESIZE → CROSS-REVIEW independently and blind to the other two workers. `gaps found` is not a single fixed rollback target and is scoped to the worker that found the gap. Evidence gaps return that worker to `RESEARCH`, synthesis gaps return that worker to `SYNTHESIZE`, and cross-source or reference-validation gaps return that worker to `CROSS-REVIEW`. A gap found inside one worker never rolls back the other two workers.

## State Definitions

### IDLE

No active research. Waiting for a research question or topic.

### RESEARCH

- **Entry**: Research question or topic identified.
- **Worker resolution**: ORCHESTRATOR resolves the three workers before dispatch. `RESEARCHER#0` is always a native subagent, has no config key, and is never looked up in `executorRouting`. `RESEARCHER#1` and `RESEARCHER#2` resolve independently: an explicit `research."1"` / `research."2"` entry beats `executors.agy` preference (no `effort` on the agy route), which beats a native subagent. Zero config resolves all three to native subagents.
- **Actions** (performed independently by each of the three workers, blind to the other two):
  - Read `~/.gal/config/config.json` when it exists before resolving output language; if `planLanguage` is set and there is no explicit override, the research narrative MUST use that language.
  - Define the research question clearly.
  - Identify candidate sources: codebase, documentation, repo-owned state, web, and optional external notes.
  - **Local-first**: Search codebase first, then repo docs, then repo-owned state via the local-first-search contract before any external source.
  - Consult external notes only when the first three lanes are insufficient and a local-notes backend is `ready`.
  - **Optional-capability preflight**: if the task maps to OpenCLI, resolve its state through the five-state preflight in [optional-capabilities.md](../conventions/optional-capabilities.md) before attempting structured retrieval. If OpenCLI is unavailable or not ready, fall back to MCP retrieval or browser tools.
  - Use Playwright MCP only when the source requires rendering or interaction that local docs, OpenCLI, fetch, or Defuddle cannot provide directly.
  - If Playwright MCP is used, capture reverse-checkable evidence such as the final URL, key interaction, and screenshot/snapshot or quoted rendered output.
  - Collect raw findings with source attribution.
  - For `deep-research`, attempt at least 5 sources and record failed search paths if the pool is constrained.
  - Write the worker's brief to `.dev/pipeline/receipts/<slug>/brief-<n>.md`. A routed worker (`#1`/`#2` resolved to a headless executor) writes this file itself, beside its own receipt. A subagent worker (`#0`, or `#1`/`#2` resolved to native) returns its brief to ORCHESTRATOR, which persists it to the same path.
- **Golem**: researcher (three parallel instances, `RESEARCHER#0`/`RESEARCHER#1`/`RESEARCHER#2`)
- **Exit**: All three workers have collected sufficient raw material to answer the question or move into structured synthesis.

### SYNTHESIZE (`deep-research`)

- **Entry**: Raw findings collected by all three workers.
- **Actions** (performed independently by each worker, still blind to the other two):
  - Organize findings into a coherent structure.
  - Identify conflicts, gaps, and open questions.
  - Produce a comparison table or decision matrix when evaluating options.
  - Separate evidence, interpretation, and recommendation.
- **Golem**: researcher (three parallel instances)
- **Exit**: Each worker has a draft synthesis ready for its own cross-source review.

### CROSS-REVIEW (`deep-research`)

- **Entry**: Synthesis complete and ready for source-to-source challenge.
- **Actions** (performed independently by each worker, still blind to the other two — this is intra-worker cross-checking against that worker's own gathered sources, not cross-worker comparison):
  - Cross-check claims against other gathered sources within the same worker.
  - Surface consensus, contradiction, and likely bias.
  - Identify where one source is being over-weighted.
  - Loop back if additional research is needed to resolve contradictions.
- **Golem**: researcher (three parallel instances)
- **Exit**: Each worker's cross-reviewed synthesis, captured in its brief, is ready for ORCHESTRATOR's independent reference verification and cross-brief adjudication.

### VERIFY

- **Entry**: All three workers' briefs are collected and ready to be citation-checked and adjudicated.
- **Actions**:
  - Reverse-check every cited reference across all three briefs. Worker isolation is the independence guarantee; using an **independent model** for this step strengthens it further, it is not required.
  - Confirm that each cited source exists and that the cited claim is supported by the source.
  - Remove or mark any dead, fabricated, or unsupported reference.
  - **Adjudicate each claim across the three briefs into exactly one claim class**, deciding on verified citations, never on how many workers happen to agree:
    - **CONSENSUS** — two or more workers make the same claim and at least one supporting citation for it verifies. Agreement across workers raises checking priority; it does not by itself make a claim true, so the citation still has to verify.
    - **DIVERGENCE** — workers disagree and the disagreement cannot be resolved by citation strength alone. Record both positions in the document and mark the unresolved claim `[UNVERIFIED]`.
    - **UNIQUE** — only one worker makes the claim, and its citation verifies. Retain the claim and label it single-source in the comparison record.
  - A claim with a verified citation from one worker outweighs an unverified or citation-free claim repeated by two or three workers. Vote count never substitutes for citation verification.
  - Produce an explicit verification summary for the final document in the same resolved language as the research narrative.
- **Model role**: reference verification by an independent model (≠ research author; orchestrator-owned, not a separate dispatch)
- **Exit**: All retained references are verified, every claim carries a claim class, and unresolved items are explicitly marked `[UNVERIFIED]`.

### DOCUMENT

- **Entry**: Research complete, references verified, and every claim adjudicated.
- **Actions**:
  - Resolve the report language by explicit directive, machine-local `planLanguage` (`config.json`), prompt-language auto-detect, then fallback `en`.
  - Determine output destination:
    - **Repo-appropriate** → `.dev/research/<slug>.md` (stays in the code repo)
    - **Private capture** → user-owned external notes only when an opted-in local-notes backend is applicable
    - **Reusable insight** → user-owned external notes only when an opted-in local-notes backend is applicable
    - **No durable write** → return the verified brief to the user without storing it
  - Write the output following the appropriate format, using the resolved language for narrative sections throughout the document.
  - Preserve source titles, URLs, and quoted original-language evidence exactly as cited.
  - Include a reference verification section or equivalent evidence note.
  - **Include the two mandatory sections** (both required, in every research document produced under this workflow):
    - **Per-worker execution evidence** — one row per worker (`RESEARCHER#0`, `RESEARCHER#1`, `RESEARCHER#2`) naming its mode (`routed` or `subagent`) and its identity: a routed worker names executor, model, effort, winning resolution rule, and executor session id; a subagent worker names its agent contract path and that it ran as a native subagent. The mode field is mandatory in both shapes. A worker whose brief could not be loaded is reported as absent in this table, never silently dropped.
    - **Comparison record** — the per-claim adjudication from VERIFY (CONSENSUS / DIVERGENCE / UNIQUE) plus the per-reference verification verdict for every retained citation.
  - For external-note writes, route only through an explicit, ready, machine-local note backend contract.
- **Exit**: Output written and confirmed, carrying both mandatory sections.

### DONE

- **Entry**: Documentation complete.
- **Actions**:
  - Update `.dev/state.md` if research was tracked as active work.
  - Link research output from relevant plan files if connected to Coding Flow work.
- **Exit**: Back to IDLE.

## Output Destinations

| Content Type | Destination | Agent | Auth Required? |
| --- | --- | --- | --- |
| Technical investigation tied to a repo | `.dev/research/` | Direct write | No |
| Private or personal research note | user-owned external notes | machine-local note backend | No |
| Reusable knowledge (patterns, models, concepts) | user-owned external notes | machine-local note backend | Yes (`start-implementation`) |
| Literature summary (from external sources) | user-owned external notes | machine-local note backend | Yes (`start-implementation`) |
| Disposable investigation | reply only, no durable write | User / any model | No |

### Routing Rule

**Default to repo** (`.dev/research/`). Route away from the repo only when one of these is true:

- The user explicitly asks for a private note or personal capture
- The findings are reusable across projects and worth keeping as long-term knowledge
- The user explicitly asks not to create a durable artifact in the repo

## Agent Activation

| State | Primary Agent | Supporting Agents |
| --- | --- | --- |
| RESEARCH | researcher × 3 (`RESEARCHER#0`/`#1`/`#2`, parallel, blind) | — |
| SYNTHESIZE | researcher × 3 (`RESEARCHER#0`/`#1`/`#2`, parallel, blind) | — |
| CROSS-REVIEW | researcher × 3 (`RESEARCHER#0`/`#1`/`#2`, parallel, blind) | — |
| VERIFY | reference check + cross-brief adjudication (independent model preferred, not required) | ORCHESTRATOR |
| DOCUMENT (repo) | User / any model | — |
| DOCUMENT (external notes) | machine-local note backend | — |

## Integration with Coding Flow

The Research Flow can feed into the Coding Flow:

1. **Pre-plan research**: Investigate before creating a plan. Research output becomes input to `/planning`, `/deep-planning`, or the current planning author.
2. **Mid-IMPLEMENT research**: Uncover unknowns during implementation. Research output goes to `.dev/research/` and is referenced in the plan.
3. **Post-verify extraction**: Verified findings worth keeping may be routed to an explicit external-note backend, but repo-owned research remains the default.

Research Flow tasks do NOT require a plan file. They are tracked informally unless the user creates one.

## Local-First Principle

Before researching externally, always check:

1. **Codebase** — source files, tests, and local comments
2. **Codebase docs** — `docs/` folder in the current repo
3. **Repo-owned state** — `.dev/`, existing plan files, and prior repo research
4. **Optional external notes** — only when repo-local retrieval is insufficient and the local-notes lane is `ready`

This prevents duplicate work and keeps research grounded in repo-owned evidence before any optional personal note system is consulted.
