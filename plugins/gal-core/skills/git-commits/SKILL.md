---
name: git-commits
description: Write commit messages that follow this project's Conventional Commit format. Use whenever the user asks for a commit message, says git-commit-msg, git-commit, or git commit, says commit msg or commit message, asks to summarize staged changes, says follow or fallow the git-commits, or needs wording that matches GAL rules. Analyze staged changes or the provided diff, choose the right commit type, detect breaking-change risk, choose the most relevant scope, and return the repo's scoped format.
---

# Git Commits Skill

## Current State

!`git diff --cached --stat` !`git log --oneline -5` !`git status --short`

## Overview

Use `gal commit-msg --print` as the deterministic baseline for `type` and `scope`, then author a descriptive subject and body from the actual staged diff. The Rust `gal commit-msg` command (in `crates/gal-engine`) replaces the retired `Get-StagedCommitMessage.ps1` / `get-staged-commit-message.sh` helpers.

The baseline is a path-and-status classifier: it gets the conventional-commit `type` and `scope` right (no-hijack — never inspects the diff body), but its subject is intentionally generic (e.g. `refactor crates`). Do not ship that generic subject verbatim — read the staged diff and write a subject that says **what actually changed** (e.g. `rename gal-core crate to gal-engine`), keeping the baseline's `type(scope)` prefix unless it is clearly wrong.

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

## Prerequisites

- Git repository initialized in the working directory
- Changes staged via `git add`, or the user has provided a diff or patch directly
- Understanding that this repo's commit style is the repo standard, including a scoped header when a primary area can be identified

## Instructions

1. Run `gal commit-msg --print` to get the baseline `type(scope)` header. If `gal` is not on PATH, fall back to manual classification (step 6).
2. Read the staged diff and write a subject that describes what actually changed, keeping the baseline's `type(scope):` prefix unless the diff shows it is wrong.
3. Do not add explanations, markdown fences, reasoning tags, JSON, or extra prose around the message.
4. Add a body (up to three bullets) only for broader changes, summarizing the most important changes — not file names.
5. Never ship the baseline's generic subject (e.g. `refactor crates`, `update scripts`) verbatim when the diff supports something more specific.
6. In fallback mode, inspect staged diffs or the provided patch to classify the commit type based on the nature of changes:
    - feat: new functionality visible to users
    - fix: bug correction
    - refactor: code restructuring without behavior change
    - docs: documentation only
    - test: adding or updating tests
    - chore: build process, dependencies, or tooling
    - perf: performance improvement
    - ci: CI/CD configuration changes
7. In fallback mode, determine scope from the primary directory or module affected (e.g., auth, api, cli, db).
8. In fallback mode, check for breaking changes: removed public APIs, changed function signatures, renamed exports, schema migrations.
9. In fallback mode, check recent commit history with git log --oneline -10 to match the project's style conventions.
10. In fallback mode, construct the commit message: type(scope): imperative description under 72 characters.
11. In fallback mode, add a body only for broader changes, with at most three short bullets describing the most important changes and their impact.
12. Do not use footers. If the change is breaking, make that clear in the header or bullets unless the user explicitly asks for generic Conventional Commits.
13. If staged changes exist, do not ask the user to describe the changes first.
14. If the request names this skill or obviously refers to commit-message generation, default to producing the commit message immediately.

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

Commit message following this repository's format:

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

## Error Handling

| Error | Cause | Solution |
| --- | --- | --- |
| `No changes staged for commit` | Nothing added to the staging area | Run `git add <files>` to stage changes before generating the message |
| `Not a git repository` | Working directory is not inside a git repo | Run `git init` or navigate to the repository root |
| `Ambiguous commit type` | Changes span multiple categories | Split into separate commits or choose the dominant intent |
| `Scope is unclear` | Changes touch many unrelated areas | Use the most significant module or feature area; omit scope only if no honest scope fits |
| `Commit message is too long` | Description is too verbose | Shorten the header and move only the most important changes into up to three bullets |

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

1. Use `gal commit-msg --print` for the `type(scope)` baseline, then author the subject/body from the staged diff.
2. Return only the commit message unless the user asks for explanation.
3. Keep the baseline `type(scope)` prefix unless the diff clearly contradicts it.
4. Fall back to fully manual diff analysis only when `gal` is unavailable or the user supplied an external diff.
