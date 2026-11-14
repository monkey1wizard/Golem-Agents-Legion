---
name: correct
description: Fix a correction and prevent evidenced recurring mistakes at the strongest suitable level.
---

# Correct recurring mistakes

When a correction arrives during work with implementation authority, fix the current mistake first. Keep the original task's file scope, dispatch ownership, and Protected Paths rules. Handle anything outside that authority through the original workflow and record it as not fixed.

## Two independent occurrences

A mistake recurs only when two independent incidents violate the same rule. Record the original source of each incident and the rule that existed at the time. Translations, generated files, summaries, and retries of the same incident do not add occurrences. Treat a single incident as an observation, not proof of recurrence.

For example, separately recommending squash, rebase, `--ff-only`, or `--no-ff` against an existing rule counts only when the incidents are independent. <!-- allow: git-integration -->

## Fix level order

Consider these levels in order and explain why each higher level is unsuitable before selecting a lower one:

1. Eliminate the mistake through architecture.
2. Exclude it through types.
3. Block it with lint or CI, such as `tools/writing/check-recurring.mjs`.
4. Detect it with a behavioral test.
5. Document a rule that requires judgment.

## Prove with history

For a machine-checkable mechanism, add tests that fail on each real historical sentence and pass on its corrected sentence. Run the tests with the existing local and CI checks.

## Record

When recurrence is proven, add the mechanism and its tests during the authorized work. Record the incident sources, the mechanism location, and its level in the `Recurring Mistakes` section of the mistake register. If only judgment can address the mistake, write `judgment` and the reason in the Correction field. Wrap-up and finalize retain their existing registration and deduplication flows.

After writing the mistake register, create a version-1 JSON input for the correction using the register's structured fields. Run `gal usage correction --input <json-path>`. If registration fails, report the failure without changing the original operation result.
