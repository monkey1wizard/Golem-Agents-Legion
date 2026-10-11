# Writing Quality

This neutral contract covers all human-facing messages and durable prose, independent of editor, provider, package, or personal skill.

## Priority and meaning

Put accuracy before clarity, and clarity before brevity. Preserve actors, conditions, negation, causality, scope, uncertainty, and relationships needed to understand or act. Write for the actual recipient. Keep background they need; remove repetition only when they already have the information. Use ordinary wording instead of internal instruction labels. Lists, sentence length, repeated subjects, and punctuation are contextual review prompts, never evidence of AI authorship.

Compare each material factual assertion in the actual unsent draft with specific support in the prompt, supplied materials, or tool observations actually made in this task. Remove or qualify unsupported claims. Missing completion evidence does not establish that work never started. Distinguish observations from inferences, assumptions, suggestions, placeholders, and future commitments. Do not invent causes, locations, roles, counts, statuses or commitments, or present suggestions as completed actions. Include only recipient-useful tool details. Honor artifact-only requests unless a higher-priority host instruction requires otherwise.

Preserve commands, code, paths, URLs and link targets, front matter, machine anchors and IDs, receipts, and literal quotations. Review ordinary table prose. Project terminology and meaning take precedence over personal preferences.

## Before delivery

Before every message, the current agent reviews the actual unsent draft in-process for accuracy, readability, and recipient needs. Include main replies, progress messages, specialist handbacks, and documents. When a configured checker is available, run it on that draft with the selected locale and profile. Do not assign a separate reviewer to every message. Instructions do not prove execution or host interception.

Use [optional-capabilities.md](optional-capabilities.md) and preserve its five availability states and outcomes. If an optional check is unavailable or fails, review the draft yourself and disclose the limitation once. Repeat the disclosure only when availability or its delivery effect changes. Do not emit progress solely to announce a check or recursively check checker-status output.

Apply automatic fixes only under an explicit safe-fix configuration. Allow at most two careful repairs, checking again after each. Undo any repair that changes facts, uncertainty, protected content, or project terms. If only advisory findings remain, deliver the last meaning-preserving draft and explain unresolved advice when relevant. A required artifact gate blocks delivery on hard findings or operational failure, including an unavailable checker, crash, or timeout. State the issue and needed action. Advisory findings alone do not block that gate unless an existing workflow explicitly requires another criterion. The current orchestrator reviews unresolved meaning and states genuine uncertainty.

## Optional tools

When chosen, textlint is the primary deterministic checker. Select each run's locale and profile. Official MCP and local CLI routes are optional; GAL use does not require Node.

zhtw-mcp supplies optional Taiwan Traditional Chinese advice, not external fact verification by default. Project terms and locale terminology profiles take precedence. No pangu or Vale service is required.

`accurate-answer` is reference material only. Core requires no private checkout or package and provides no such skill. Personal Conventions may add compatible preferences. When absent or off, no personal instructions are added; this neutral minimum still applies. Downstream projects own their optional profiles and vocabularies.

## Workflow boundary

Preserve `PROSE_AUDIT: required`, ownership, `not-run`, `STOP`, machine headings, and receipt placement. Checker evidence never replaces semantic review or existing workflow stop rules.
