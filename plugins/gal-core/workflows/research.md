# Research Flow

A workflow for research-driven tasks: investigating technologies, checking evidence quality, and writing durable results with repo-owned outputs first and optional external-note routing only when explicitly applicable.

`golem-researcher` owns the RESEARCH, SYNTHESIZE, and CROSS-REVIEW states. Reference verification must be performed by an independent model that did not author the research findings. Any optional external-note write is routed through a machine-local note backend rather than a public vault-writer role.

This workflow is **independent of the Coding Flow**. It can run in parallel with active development, or as a standalone investigation.

## Modes

| Mode | When | States | Minimum source rule | Verification rule |
| --- | --- | --- | --- | --- |
| `research` | Standard structured investigation | RESEARCH → VERIFY → DOCUMENT | Gather enough sources to answer the question responsibly | Every cited reference must be reverse-checked by an independent model |
| `deep-research` | Higher-risk, higher-ambiguity, or cross-cutting investigation | RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT | Attempt at least 5 sources | Every cited reference must be reverse-checked by an independent model |

## Mandatory Rules

1. **Local-first remains mandatory**. Check codebase, repo docs, and repo-owned state before external sources; external notes are optional add-on evidence only.
2. **Every cited reference must exist**. Missing, dead, or unverifiable references must be removed or explicitly marked `[UNVERIFIED]` before documentation.
3. **Reference verification must be independent**. The model that performs VERIFY must not be the same model that authored the research findings.
4. **`deep-research` must attempt at least 5 sources**. If fewer than 5 are available, document the failed search attempts and why the source pool is constrained.
5. **CROSS-REVIEW is about source-to-source consistency**. It is not a substitute for architecture, business, or design review.
6. **Playwright MCP is a dynamic-page aid, not a research default**. Use it only after local-first and structured retrieval paths cannot answer the question.
7. **Browser-backed research must preserve reverse-checkable evidence**. Record the URL, the interaction performed, and the artifact or observation that supports the claim.
8. **Research narrative follows one resolved output language**. Resolve it by explicit directive, machine-local `planLanguage` (`config.json`), prompt-language auto-detect, then fallback `en`.
9. **Keep the report fully in the resolved language**. Do not mix prose languages inside one research document, except for literal technical identifiers and preserved source material.
10. **Preserve citations in their original form**. Source titles, URLs, quoted snippets, and other directly cited original-language material stay unchanged even when the report narrative is translated.

## State Machine

### `research`

```text
IDLE → RESEARCH → VERIFY → DOCUMENT → DONE
```

### `deep-research`

```text
IDLE → RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT → DONE
         ↑            ↑             ↑             │
         └─ evidence ─┴─ synthesis ─┴─ source/ref ┘
```

`gaps found` is not a single fixed rollback target. Evidence gaps return to `RESEARCH`, synthesis gaps return to `SYNTHESIZE`, and cross-source or reference-validation gaps return to `CROSS-REVIEW`.

## State Definitions

### IDLE

No active research. Waiting for a research question or topic.

### RESEARCH

- **Entry**: Research question or topic identified.
- **Actions**:
  - Read `~/.gal/config/config.json` when it exists before resolving output language; if `planLanguage` is set and there is no explicit override, the research narrative MUST use that language.
  - Define the research question clearly.
  - Identify candidate sources: codebase, documentation, repo-owned state, web, and optional external notes.
  - **Local-first**: Search codebase first, then repo docs, then repo-owned state via the local-first-search contract before any external source.
  - Consult external notes only when the first three lanes are insufficient and a local-notes backend is `ready`.
  - **Collaborative-tool preflight**: if the task maps to OpenCLI, resolve the tool state through [docs/collaborative-tools/checking-contract.md](../docs/collaborative-tools/checking-contract.md) before attempting structured retrieval. If OpenCLI is unavailable or not ready, fall back through [opencli.md](../docs/collaborative-tools/opencli.md).
  - Use Playwright MCP only when the source requires rendering or interaction that local docs, OpenCLI, fetch, or Defuddle cannot provide directly.
  - If Playwright MCP is used, capture reverse-checkable evidence such as the final URL, key interaction, and screenshot/snapshot or quoted rendered output.
  - Collect raw findings with source attribution.
  - For `deep-research`, attempt at least 5 sources and record failed search paths if the pool is constrained.
- **Golem**: researcher
- **Exit**: Sufficient raw material collected to answer the question or move into structured synthesis.

### SYNTHESIZE (`deep-research`)

- **Entry**: Raw findings collected.
- **Actions**:
  - Organize findings into a coherent structure.
  - Identify conflicts, gaps, and open questions.
  - Produce a comparison table or decision matrix when evaluating options.
  - Separate evidence, interpretation, and recommendation.
- **Golem**: researcher
- **Exit**: Draft synthesis ready for cross-source review.

### CROSS-REVIEW (`deep-research`)

- **Entry**: Synthesis complete and ready for source-to-source challenge.
- **Actions**:
  - Cross-check claims against other gathered sources.
  - Surface consensus, contradiction, and likely bias.
  - Identify where one source is being over-weighted.
  - Loop back if additional research is needed to resolve contradictions.
- **Golem**: researcher
- **Exit**: Cross-reviewed synthesis ready for independent reference verification.

### VERIFY

- **Entry**: Research findings are ready to be citation-checked.
- **Actions**:
  - Use an **independent model** to reverse-check every cited reference.
  - Confirm that each cited source exists and that the cited claim is supported by the source.
  - Remove or mark any dead, fabricated, or unsupported reference.
  - Produce an explicit verification summary for the final document in the same resolved language as the research narrative.
- **Model role**: reference verification by an independent model (≠ research author; orchestrator-owned, not a separate dispatch)
- **Exit**: All retained references are verified, or unresolved items are explicitly marked.

### DOCUMENT

- **Entry**: Research complete and references verified.
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
  - For external-note writes, route only through an explicit, ready, machine-local note backend contract.
- **Exit**: Output written and confirmed.

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
| RESEARCH | researcher | — |
| SYNTHESIZE | researcher | — |
| CROSS-REVIEW | researcher | — |
| VERIFY | independent model (reference check) | — |
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
