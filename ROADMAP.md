# Roadmap

GAL (Golem Agents Legion) bootstrap implementation progress.

## Milestone 1: Bootstrap (v0.1)

Build the complete GAL system from scratch.

### Phases

| Phase | Name | Status | Summary |
| --- | --- | --- | --- |
| 1 | Bootstrap Repo | ✅ Done | Initial repo structure, agent files, conventions, templates, scripts |
| 1.5 | Golem Agents | ✅ Done | 9 agent definitions (planner through scribe) |
| 2 | Skills Migration | ✅ Done | 12 custom skills copied, Setup-Machine scripts, symlink verification |
| 3 | Conventions + Templates | ✅ Done | 6 convention files extracted from skills, agent template created |
| 4 | Design Sync | ✅ Done | All bootstrap design decisions extracted to implementation files |
| 5 | Per-Repo Integration | ⬜ Pending | First target repo integration test |

### Phase 4 Details (Design Sync)

| Sub-phase | What | Files Changed |
| --- | --- | --- |
| 4A | Workflow migration | `workflows/coding.md` created, `workflow.md` deleted |
| 4B | Agent + model updates | 7 agent files + `model-roles.md` updated |
| 4C | Template updates | `plan.md`, `state.md`, `project.md`, `diary.md` updated |
| 4D | README + ROADMAP | `README.md` rewritten, `ROADMAP.md` created |
| 4E | Script updates | `gal.ps1`, `gal.sh`, `Init-Repo.ps1`, `init-repo.sh` updated |
| 4F | Bootstrap termination | Bootstrap plan marked ABSORBED and deleted |

### Phase 5 Scope (Per-Repo Integration)

- Initialize `<target-repo>/.dev/project.md` (adopt-existing mode)
- Initialize `<target-repo>/.dev/state.md`
- Write `scripts/Sync-DevContext.ps1` (adapter generator)
- Verify `gal init/plan/status/next/pause` on Windows + macOS
- Run Sync → verify generated `copilot-instructions.md` / `GEMINI.md`
- Run full coding flow (plan → implement → test → review → verify) on a real feature

## Future Milestones

### v0.2: Research Flow

- Define `workflows/research.md` state machine
- Add researcher golem agent
- Define research output format (ADR, comparison tables)

### v0.3: Multi-Machine Sync

- Verify full cross-machine workflow across two or more machines
- Test async plan handoff between machines
- Ollama model routing for LOCAL role on each machine

### v0.4: Community & Docs

- Getting-started guide for other solo developers
- Contribution guidelines
- Example adapter templates for popular tools
