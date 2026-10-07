---
name: git-commits
description: Write commit messages that follow this project's Conventional Commit format, and when the user's intent is to actually commit staged changes, generate the message via git-commit-msg rules and execute git commit with that exact message. Use whenever the user asks for a commit message, says git-commit-msg, git-commit, or git commit, says commit msg or commit message, asks to summarize staged changes, says follow or fallow the git-commits, or needs wording that matches GAL rules. Analyze staged changes or the provided diff, choose the right commit type, detect breaking-change risk, choose the most relevant scope, and either return the repo's scoped format or commit with it depending on intent.
---

# Git Commits Skill

## Current State

!`git diff --cached --stat` !`git log --oneline -5` !`git status --short`

## Overview

Use `gal commit-msg --context` as the single front-end call: it does all necessary pre-work and returns up to five blocks — `FILES` (name-status), `BASELINE` (the deterministic `type`/`scope`), `PLANS` (per staged plan: slug, verb, title, Goal), `PROMPTS` (staged execution-prompt slugs), and `CHANGES` (hunk headers for non-plan code files). The Rust `gal commit-msg` command lives in `crates/cli/src/gal/commit_msg.rs` and replaces the retired `Get-StagedCommitMessage.ps1` / `get-staged-commit-message.sh` helpers.

`BASELINE` is a path-and-status classifier: it gets the conventional-commit `type` and `scope` right (no-hijack — never inspects the diff body), but its subject is intentionally generic (e.g. `refactor crates`). Do not ship that generic subject verbatim — use `PLANS`'s Goal (for staged plan files) or `CHANGES`'s hunk headers (for code files) to write a subject that says **what actually changed** (e.g. `rename gal-core crate to gal-engine`), keeping the baseline's `type(scope)` prefix unless it is clearly wrong.

This repo's local rules override generic Conventional Commits guidance when they conflict. The final output must use a scoped header and stay in this repo's no-footer format unless the user explicitly asks for generic Conventional Commits instead.

## Activation Rule

Treat all of the following as requests for this skill, even when the wording is casual, ungrammatical, abbreviated, or partially incorrect:

- "fallow the git-commits to give me commit msg"
- "git-commit"
- "git-commit-msg"
- "git commit message"
- "give me commit msg"
- "write commit msg"
- "summarize staged changes into commit message"
- "help me commit this"

If the user intent is clearly to produce commit-message wording from staged changes or a provided diff, activate this skill instead of asking a broad clarifying question.

Treat requests as **commit intent** when the user is asking to perform the commit, not merely draft wording. Examples: "git-commit", "commit this", "help me commit this", "use git-commits and commit", "generate the message and commit directly".

Treat requests as **message-only intent** when the user asks for a message, summary, wording, or `/git-commit-msg` behavior without asking to execute the commit.

## Prerequisites

- Git repository initialized in the working directory
- Changes staged via `git add`, or the user has provided a diff or patch directly
- Understanding that this repo's commit style is the repo standard, including a scoped header when a primary area can be identified

## Instructions

1. Run `gal commit-msg --context` once to get `FILES`/`BASELINE`/`PLANS`/`PROMPTS`/`CHANGES`. If `gal` is not on PATH, fall back to manual classification (step 10).
2. Write a subject that describes what actually changed, using `PLANS`'s Goal for staged plan files and `CHANGES`'s hunk headers for code files — do not read the full raw diff yourself; the context call already extracted the high-signal parts. Keep `BASELINE`'s `type(scope):` prefix unless the context clearly shows it is wrong.
3. If the intent is message-only, return the final commit message only.
4. If the intent is commit, use the final generated message as the exact `git commit` message and execute the commit instead of only returning the text.
5. For commit intent, preserve the full message exactly. For multi-line messages, prefer `git commit --file <tempfile>` or an equivalent method that preserves newlines rather than collapsing the body.
6. Do not add explanations, markdown fences, reasoning tags, JSON, or extra prose around the message when producing the commit text.
7. Add a body (up to three bullets) only for broader changes, summarizing the most important changes — not file names.
8. Never ship the baseline's generic subject (e.g. `refactor crates`, `update scripts`) verbatim when the diff supports something more specific.
9. **Context guard**: Even when the staged diff contains GAL plan files (`.dev/state.md`, `.dev/plans/*.md`, `.dev/plans/*.md`) or any other GAL infrastructure files, you are writing a **commit message** — do not produce a status report, plan summary, or active-plans listing. Stay in commit-message mode regardless of the diff content.
10. In fallback mode, inspect staged diffs or the provided patch to classify the commit type based on the nature of changes:
    - feat: new functionality visible to users
    - fix: bug correction
    - refactor: code restructuring without behavior change
    - docs: documentation only
    - test: adding or updating tests
    - chore: build process, dependencies, or tooling
    - perf: performance improvement
    - ci: CI/CD configuration changes
11. In fallback mode, determine scope from the primary directory or module affected (e.g., auth, api, cli, db).
12. In fallback mode, check for breaking changes: removed public APIs, changed function signatures, renamed exports, schema migrations.
13. In fallback mode, check recent commit history with git log --oneline -10 to match the project's style conventions.
14. In fallback mode, construct the commit message: type(scope): imperative description under 72 characters.
15. In fallback mode, add a body only for broader changes, with at most three short bullets describing the most important changes and their impact.
16. Do not use footers. If the change is breaking, make that clear in the header or bullets unless the user explicitly asks for generic Conventional Commits.
17. If staged changes exist, do not ask the user to describe the changes first.
18. If the request names this skill or obviously refers to commit-message generation, default to producing the commit message immediately unless the wording clearly requests an actual commit.
19. If the request clearly asks to commit, default to executing the commit after generating the final message.

## Rules

- Use imperative mood: `add feature`, not `added feature`
- Start with lowercase
- Do not end the subject with a period
- Keep the header under 72 characters when practical
- Describe what changed, not the implementation process
- Use scope in the header: `feat(core):` not `feat:` when a primary area can be identified
- Choose the most relevant scope from the dominant module, directory, command, or feature area
- Prefer repo helper output over model-authored summaries whenever the helper exists
- Do not add a body when the helper returned header only
- Do not add footer lines such as `Closes #123`
- Do not use `BREAKING CHANGE:` footers unless the user explicitly asks for generic Conventional Commits instead of this repo's format
- If the change is breaking, make that obvious in the title or bullets without introducing a footer unless the user explicitly asks for one

## Output

For message-only intent, produce a commit message following this repository's format:

```text
<type>(<scope>): <brief description>
```

For broader changes, use this format with at most three bullets:

```text
<type>(<scope>): <brief description>

- most important change
- most important change
- most important change
```

Bullet rules:

- Use zero to three bullets only
- Summarize the most important changes, not file names or file lists
- Keep each bullet short and high signal
- Omit bullets entirely for small, single-purpose changes

For commit intent:

- Generate the final message using the same format and rules as message-only intent
- Execute `git commit` with that exact message
- Return a concise success or failure result rather than re-explaining the formatting rules

## Error Handling

| Error | Cause | Solution |
| --- | --- | --- |
| `No changes staged for commit` | Nothing added to the staging area | Run `git add <files>` to stage changes before generating the message |
| `Not a git repository` | Working directory is not inside a git repo | Run `git init` or navigate to the repository root |
| `Ambiguous commit type` | Changes span multiple categories | Split into separate commits or choose the dominant intent |
| `Scope is unclear` | Changes touch many unrelated areas | Use the most significant module or feature area; omit scope only if no honest scope fits |
| `Commit message is too long` | Description is too verbose | Shorten the header and move only the most important changes into up to three bullets |
| `git commit` failed | Hook rejected the commit, nothing was staged, or the worktree changed during commit | Return the git error succinctly and do not claim success |

## Examples

- "Analyze my staged changes and generate a scoped commit message in this repo's format."
- "Create a commit message for these changes and call out any breaking-change risk without using a footer."
- "Summarize this broader change as a scoped title plus the three most important changes."
- "fallow the git-commits to give me commit msg"
- "git-commit-msg for current staged changes"
- "git-commit for current staged changes"
- "give me commit msg from staged diff"

## Resources

- [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/)
- [Git commit best practices](https://cbea.ms/git-commit/)

## Output Requirements

When asked for a commit message:

1. Use `gal commit-msg --context` for the `FILES`/`BASELINE`/`PLANS`/`PROMPTS`/`CHANGES` blocks, then author the subject/body from them.
2. Return only the commit message unless the user asks for explanation or clearly asks to execute the commit.
3. Keep the `BASELINE` `type(scope)` prefix unless the context clearly contradicts it.
4. Fall back to fully manual diff analysis only when `gal` is unavailable or the user supplied an external diff.

When asked to commit:

1. Generate the message using the same `git-commit-msg`-aligned rules.
2. Execute `git commit` with that exact final message.
3. Report whether the commit succeeded, and include the final commit message only when useful for confirmation.
4. Do not silently downgrade a commit request into message-only output.
