# Research Flow

A workflow for research-driven tasks — investigating technologies, synthesizing findings, and writing results to the code repo (`docs/`) or Obsidian vault via the librarian.

`golem-researcher` owns the RESEARCH and SYNTHESIZE states. `architect`, `analyst`, and `designer` remain review specialists; `librarian` remains the only vault writer.

This workflow is **independent of the Coding Flow**. It can run in parallel with active development, or as a standalone investigation.

## Tier System

| Tier | When | States | Output | Librarian? |
| --- | --- | --- | --- | --- |
| R0 (Quick) | Single-source lookup, known answer expected | RESEARCH → DOCUMENT | Brief note in `docs/` or chat reply | No |
| R1 (Standard) | Multi-source investigation, comparison needed | RESEARCH → SYNTHESIZE → DOCUMENT | Research doc in `docs/research/` or `10_Projects/` | Optional |
| R2 (Deep) | Unknown domain, multi-day, cross-cutting | RESEARCH → SYNTHESIZE → REVIEW → DOCUMENT | Full research report + vault knowledge extraction | Yes |

**Upgrade rule**: Any tier can upgrade to R2 if scope expands. Stop, reassess, and adjust.

## State Machine

### R2 (Full)

```text
IDLE → RESEARCH → SYNTHESIZE → REVIEW → DOCUMENT → DONE
                                  ↑                   │
                                  └── (gaps found) ───┘
```

### R1 (Standard)

```text
IDLE → RESEARCH → SYNTHESIZE → DOCUMENT → DONE
```

### R0 (Quick)

```text
IDLE → RESEARCH → DOCUMENT → DONE
```

## State Definitions

### IDLE

No active research. Waiting for a research question or topic.

### RESEARCH

- **Entry**: Research question or topic identified.
- **Actions**:
  - Define the research question clearly.
  - Identify sources: documentation, codebase, web, existing vault notes.
  - **Local-first**: Always search the Obsidian vault first (via local-first-search skill) before external sources.
  - Collect raw findings with source attribution.
  - For R1/R2: create a working document to track progress.
- **Golem**: researcher
- **Exit**: Sufficient raw material collected to answer the question.

### SYNTHESIZE (R1/R2)

- **Entry**: Raw findings collected.
- **Actions**:
  - Organize findings into coherent structure.
  - Identify conflicts, gaps, and open questions.
  - For R2: produce a comparison table or decision matrix if evaluating options.
  - Cross-reference with existing vault knowledge.
- **Golem**: researcher
- **Exit**: Draft synthesis ready for review (R2) or documentation (R1).

### REVIEW (R2 only)

- **Entry**: Synthesis complete, needs adversarial review.
- **Actions**:
  - Architect reviews (conditional) for technical accuracy, missing trade-offs, and bias.
  - Analyst reviews (conditional) if business implications exist.
  - Designer reviews (conditional) if the research affects visual design, UX flow, accessibility, or design systems.
  - Identify gaps that need additional research → loop back to RESEARCH.
- **Golems**: architect (conditional), analyst (conditional), designer (conditional)
- **Exit**: All reviewers satisfied, no critical gaps remain.
- **Checkpoint**: HUMAN — confirm findings before documentation.

### DOCUMENT

- **Entry**: Research complete (R0) or synthesis reviewed (R1/R2).
- **Actions**:
  - Determine output destination:
    - **Repo-appropriate** → `docs/research/<slug>.md` (stays in the code repo)
    - **Project knowledge** → `10_Projects/` via librarian (requires `start-implementation`)
    - **Reusable insight** → `20_Slipbox/` via librarian (requires `start-implementation`)
  - Write the output following the appropriate format.
  - For vault writes: librarian handles all formatting, naming, and Guide.md compliance.
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
| Quick answer (R0, ephemeral) | Chat reply or plan notes | Direct | No |

### Routing Rule

**Default to repo** (`docs/research/`). Only route to Obsidian when the knowledge is:

- Reusable across projects (not repo-specific)
- Worth maintaining long-term (not disposable investigation notes)
- The user explicitly requests vault storage

## Agent Activation

| State | Primary Agent | Supporting Agents |
| --- | --- | --- |
| RESEARCH | researcher | — |
| SYNTHESIZE | researcher | — |
| REVIEW | architect (conditional) | analyst (conditional), designer (conditional) |
| DOCUMENT (repo) | User / any model | — |
| DOCUMENT (vault) | librarian | — |

## Integration with Coding Flow

The Research Flow can feed into the Coding Flow:

1. **Pre-plan research**: Investigate before creating a plan. Research output becomes input to `/office-hours` or the current planning author.
2. **Mid-IMPLEMENT research**: Uncover unknowns during implementation. Research output goes to `docs/research/` and is referenced in the plan.
3. **Post-VERIFY extraction**: Verifier identifies knowledge worth extracting → triggers librarian for vault writes.

Research Flow tasks do NOT require a plan file. They are tracked informally unless the user creates one.

## Local-First Principle

Before researching externally, always check:

1. **Obsidian vault** — semantic search via local-first-search skill
2. **Codebase docs** — `docs/` folder in the current repo
3. **Existing plan files** — previous research that may already cover the topic

This prevents duplicate work and respects the Context-First philosophy from Guide.md.
