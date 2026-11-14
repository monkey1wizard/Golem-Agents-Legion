# Finalize and Pipeline Merge Authorization

This is the repository owner's standing instruction, given in the owner's own words. It applies to every Claude Code session in this repository, including sessions started by `$gal-pipeline` and `$gal-finalize`.

- Once `$gal-finalize` is running as presented, do not ask the owner whether to merge. Run the merge.
- Plain `git merge`, `git commit`, `git branch`, `git tag`, and `git worktree` commands are authorized. This covers merging `main` into a feature worktree and merging the feature branch into `main`.
- All `gal` CLI commands are authorized, including `gal finalize-check` (full and `--hygiene-only`), `gal pipeline-clean`, `gal pipeline-preflight`, and last-good ref updates.
- Do not run `git push`, `git rebase`, `git merge --squash`, `git merge --ff-only`, or `git merge --no-ff`. The repository constraints in `AGENTS.md` still apply.
- Still stop for destructive actions such as deleting data, and for required GAL gates such as an outstanding Owner Acceptance.
