# Research Flow

A workflow for research-driven tasks: investigating technologies, checking evidence quality, and writing durable results to the repo (`docs/`) or Obsidian vault via the librarian.

`golem-researcher` owns the RESEARCH, SYNTHESIZE, and CROSS-REVIEW states. Reference verification must be performed by an independent model that did not author the research findings. `librarian` remains the only vault writer.

This workflow is **independent of the Coding Flow**. It can run in parallel with active development, or as a standalone investigation.

## Modes

| Mode | When | States | Minimum source rule | Verification rule |
| --- | --- | --- | --- | --- |
| `research` | Standard structured investigation | RESEARCH → VERIFY → DOCUMENT | Gather enough sources to answer the question responsibly | Every cited reference must be reverse-checked by an independent model |
| `deep-research` | Higher-risk, higher-ambiguity, or cross-cutting investigation | RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT | Attempt at least 5 sources | Every cited reference must be reverse-checked by an independent model |

## Mandatory Rules

1. **Local-first remains mandatory**. Check the vault and repo docs before external sources.
2. **Every cited reference must exist**. Missing, dead, or unverifiable references must be removed or explicitly marked `[UNVERIFIED]` before documentation.
3. **Reference verification must be independent**. The model that performs VERIFY must not be the same model that authored the research findings.
4. **`deep-research` must attempt at least 5 sources**. If fewer than 5 are available, document the failed search attempts and why the source pool is constrained.
5. **CROSS-REVIEW is about source-to-source consistency**. It is not a substitute for architecture, business, or design review.

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
  - Define the research question clearly.
  - Identify candidate sources: documentation, codebase, web, and existing vault notes.
  - **Local-first**: Always search the Obsidian vault first (via local-first-search skill) before external sources.
  - **Collaborative-tool preflight**: if the task maps to OpenCLI, resolve the tool state through [docs/collaborative-tools/checking-contract.md](../docs/collaborative-tools/checking-contract.md) before attempting structured retrieval. If OpenCLI is unavailable or not ready, fall back through [opencli.md](../docs/collaborative-tools/opencli.md).
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
  - Produce an explicit verification summary for the final document.
- **Golem**: independent verifier model
- **Exit**: All retained references are verified, or unresolved items are explicitly marked.

### DOCUMENT

- **Entry**: Research complete and references verified.
- **Actions**:
  - Determine output destination:
    - **Repo-appropriate** → `docs/research/<slug>.md` (stays in the code repo)
    - **Project knowledge** → `10_Projects/` via librarian (requires `start-implementation`)
    - **Reusable insight** → `20_Slipbox/` via librarian (requires `start-implementation`)
  - Write the output following the appropriate format.
  - Include a reference verification section or equivalent evidence note.
  - For vault writes, librarian handles all formatting, naming, and Guide.md compliance.
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
| Technical investigation tied to a repo | `docs/research/` | Direct write | No |
| Project-scoped research (tied to a project) | `10_Projects/` | Librarian | Yes (`start-implementation`) |
| Reusable knowledge (patterns, models, concepts) | `20_Slipbox/22_Permanent/` | Librarian | Yes (`start-implementation`) |
| Literature summary (from external sources) | `20_Slipbox/21_Literature/` | Librarian | Yes (`start-implementation`) |

### Routing Rule

**Default to repo** (`docs/research/`). Only route to Obsidian when the knowledge is:

- Reusable across projects (not repo-specific)
- Worth maintaining long-term (not disposable investigation notes)
- Explicitly requested for vault storage

## Agent Activation

| State | Primary Agent | Supporting Agents |
| --- | --- | --- |
| RESEARCH | researcher | — |
| SYNTHESIZE | researcher | — |
| CROSS-REVIEW | researcher | — |
| VERIFY | independent verifier model | — |
| DOCUMENT (repo) | User / any model | — |
| DOCUMENT (vault) | librarian | — |

## Integration with Coding Flow

The Research Flow can feed into the Coding Flow:

1. **Pre-plan research**: Investigate before creating a plan. Research output becomes input to `/planning`, `/deep-planning`, or the current planning author.
2. **Mid-IMPLEMENT research**: Uncover unknowns during implementation. Research output goes to `docs/research/` and is referenced in the plan.
3. **Post-verify extraction**: Verified findings worth keeping can be handed to librarian for vault extraction.

Research Flow tasks do NOT require a plan file. They are tracked informally unless the user creates one.

## Local-First Principle

Before researching externally, always check:

1. **Obsidian vault** — semantic search via local-first-search skill
2. **Codebase docs** — `docs/` folder in the current repo
3. **Existing plan files** — previous research that may already cover the topic

This prevents duplicate work and respects the Context-First philosophy from Guide.md.
