# Roadmap

GAL (Golem Agents Legion) bootstrap implementation progress.

## Milestone 1: Bootstrap (v0.1)

Build the complete GAL system from scratch.

### Phases

| Phase | Name | Status | Summary |
| --- | --- | --- | --- |
| 1 | Bootstrap Repo | ✅ Done | Initial repo structure, agent files, conventions, templates, scripts |
| 1.5 | Golem Agents | ✅ Done | 10 agent definitions (planner through librarian) |
| 2 | Skills Migration | ✅ Done | 12 custom skills copied, Setup-Machine scripts, symlink verification |
| 3 | Conventions + Templates | ✅ Done | 6 convention files extracted from skills, agent template created |
| 4 | Design Sync | ✅ Done | All bootstrap design decisions extracted to implementation files |
| 5 | Per-Repo Integration | ✅ Done | Sync-DevContext shipped; field validation on macOS and the first real feature run are deferred follow-up checks |

### Phase 4 Details (Design Sync)

| Sub-phase | What | Files Changed |
| --- | --- | --- |
| 4A | Workflow migration | `workflows/coding.md` created, `workflow.md` deleted |
| 4B | Agent + model updates | 7 agent files + `model-roles.md` updated |
| 4C | Template updates | `plan.md`, `state.md`, `project.md`, `diary.md` updated |
| 4D | README + ROADMAP | `README.md` rewritten, `ROADMAP.md` created |
| 4E | golem- rename audit | 13 files updated, 27 logical edits — agent/template/script refs |
| 4F | Smudge/clean filter + hooks | `gal-smudge.sh`, `gal-clean.sh`, `.gitattributes`, `.githooks/pre-commit` |
| 4G | Gemini support + doc alignment | Setup-Machine Gemini symlinks, gal-context.md, README/ROADMAP/scripts.md aligned |

### Phase 5 Scope (Per-Repo Integration)

Phase 5 is considered complete on implementation grounds.
The remaining unchecked items are treated as deferred field validation, not blockers for closing the bootstrap.

- [x] Initialize `<target-repo>/.dev/project.md` (adopt-existing mode)
- [x] Initialize `<target-repo>/.dev/state.md`
- [x] Write `scripts/Sync-DevContext.ps1` + `sync-dev-context.sh` (adapter generators)
- [x] Run Sync and verify generated `copilot-instructions.md` / `GEMINI.md`
- [ ] Deferred follow-up: verify `gal init/plan/status/next/pause` on macOS
- [ ] Deferred follow-up: run the full coding flow (plan → implement → test → review → verify) on a real feature such as `feat-basic-mode`

### Deferred Follow-Up Checks

These checks are intentionally left for real-world use instead of blocking completion now:

- macOS parity verification for the command family
- first real-feature end-to-end validation in a target repo
- any bug fixes discovered during those field runs

## Future Milestones

### v0.2: Research Flow

- Define `workflows/research.md` state machine ✅
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
