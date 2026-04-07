---
name: git-commits
description: Write commit messages that follow this project's Conventional Commit format. Use when the user asks for a commit message, wants to rewrite one, or needs commit wording that matches GAL rules.
---

# Git Commits Skill

Write commit messages using the project's required Conventional Commit format.

## Format

```text
<type>: <brief description>
```

## Types

| Type | Description |
| --- | --- |
| `feat` | A new feature |
| `fix` | A bug fix |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `docs` | Documentation only changes |
| `style` | Formatting changes with no logic change |
| `test` | Adding or correcting tests |
| `chore` | Build process or auxiliary tools |
| `perf` | Performance improvements |

## Rules

1. Use imperative mood: `add feature`, not `added feature`
2. Start with lowercase
3. Do not end the subject with a period
4. Keep the subject under 50 characters
5. Describe what changed, not why
6. Do not use scope: `feat:` not `feat(core):`
7. Do not add footer lines such as `Closes #123`

## Decision Tree

```text
Ready to commit -> How many files changed?
    |- 1-3 files, same purpose -> Format A: one-liner
    `- 4+ files, or diverse   -> Format B: title + bullet list
```

## Format A - Minimal

Use for 1-3 files that serve the same purpose.

```text
feat: add public email verification token
fix: handle missing signup time correctly
```

## Format B - Substantial

Use for broader changes across multiple files or concerns.

```text
feat: brief description

- File/Component 1 (purpose)
- File/Component 2 (purpose)
```

### Bullet Guidelines

- Group related files on a single bullet when they share a purpose
- Use semicolons to separate multiple changes within one bullet
- Put all detail in bullets, not in a body paragraph
- Do not add a footer

## Output Requirements

When asked for a commit message:

1. Pick the correct type
2. Choose Format A or Format B using the decision tree
3. Return only the commit message unless the user asks for explanation
