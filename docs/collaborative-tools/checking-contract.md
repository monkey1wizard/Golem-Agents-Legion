# Collaborative Tool Checking Contract

This document defines the shared preflight model for GAL collaborative tools.
Use it when a workflow may optionally collaborate with graphify, OpenCLI, gstack,
or a future tool that extends GAL without becoming a core runtime dependency.

## Purpose

The workflow layer should answer the same questions in the same order before it
uses any collaborative tool:

```text
applicability -> availability -> initialization status -> readiness -> route -> degrade
```

This keeps optional tools useful without letting missing setup look like a
successful run.

## Standard States

| State | Meaning | Owned by | Expected behavior |
| --- | --- | --- | --- |
| `not-applicable` | The current lane or task does not need this tool. | workflow layer | Skip the tool without surfacing setup work. |
| `unavailable` | The machine or runtime cannot access the tool. | machine-local install and runtime wiring | Use the documented fallback path. Do not pretend the tool ran. |
| `available-but-needs-init` | The tool exists, but first-time setup for this machine, repo, or runtime is incomplete. | tool-specific init contract | Do not auto-initialize during normal planning, review, or research. |
| `available-but-not-ready` | The tool is installed and initialized, but the current repo or task lacks the artifacts needed for this lane. | repo or task preconditions | Degrade to the documented non-tool path. |
| `ready` | The tool is applicable and all required preconditions are satisfied. | workflow layer after preflight | Route into the tool-specific capability. |

## Decision Boundaries

- Applicability is a workflow concern.
- Availability is a machine-local concern.
- Initialization status is a tool-specific concern.
- Readiness is a repo or task concern.
- Routing and degradation are workflow concerns.

Personalization may affect availability and machine-local preferences. It must
not redefine workflow semantics or readiness rules.

## Required Behavior

- Do not auto-install collaborative tools.
- Do not auto-run first-time initialization during normal planning, review, or research.
- Do not hide missing capabilities behind vague success language.
- Always define an explicit degrade path for each tool-enabled lane.
- Keep the normal GAL write-back targets unchanged unless a tool contract says otherwise.

## Status Mapping Guidance

Use these distinctions when documenting or implementing a tool:

- `availability` answers whether the tool can be reached from the current machine or runtime.
- `initialization status` answers whether one-time setup has already been completed.
- `readiness` answers whether the current repo or task has the files, adapters, schema, or state required for the requested lane.

## Current Tool Mappings

- [graphify.md](graphify.md) for structural planning and review context.
- [opencli.md](opencli.md) for research-side structured external retrieval.
- [gstack.md](gstack.md) for planning-stage review lanes and specialist collaboration.
