---
name: learn
description: "Institutional memory. View, search, add, and prune learnings in .dev/learnings.jsonl. Each learning has a confidence score (0–10) and source attribution. High-confidence learnings (9+) are surfaced in /gal wrap-up handoff notes."
---

# /learn

Manage the project's accumulated learnings. Build institutional memory.

## Role

Institutional memory keeper. Make sure what was learned is not re-learned.

## When to Use

- After a difficult debugging session to record what was discovered
- After `/ship`, `/land-and-deploy`, or another high-signal workflow checkpoint to preserve reusable lessons
- When you want to search what was learned before making a recommendation
- Periodically to prune stale entries

## Store Location

`.dev/learnings.jsonl` — one JSON object per line.

Each learning:
```json
{
  "id": "<uuid>",
  "date": "<ISO date>",
  "confidence": 8,
  "text": "<actionable insight>",
  "source": "<command or event that generated this>",
  "files": ["src/api/handler.ts"],
  "tags": ["performance", "caching"]
}
```

## Commands

### `/learn` (no args) — Interactive Menu

Show:
```
Learnings: N total, M high-confidence (9+)

Options:
  1. View all
  2. Search
  3. Add new learning
  4. Prune stale entries
  5. Export for team sharing
```

### `/learn view`

List all learnings sorted by confidence (highest first). Display:
- Confidence score
- Date
- Text (truncated at 120 chars)
- Source

### `/learn search <query>`

Full-text search across learning text, tags, and file paths. Return ranked matches.

### `/learn add`

Add a new learning interactively:
1. Ask: "What was learned?" (the insight, actionable)
2. Ask: "Confidence 0–10?" (0 = hunch, 10 = verified in production multiple times)
3. Ask: "Which files does this relate to?" (optional)
4. Ask: "Tags?" (optional, comma-separated)

Write to `.dev/learnings.jsonl`.

### `/learn prune`

Find stale entries:
- Referenced files that no longer exist in the repo
- Learnings with confidence < 3 that are older than 90 days

Show the candidate list and ask: "Remove these? (y/n for each)"

### `/learn export`

Export all learnings with confidence ≥ 7 to `docs/learnings-export.md` in human-readable format. Suitable for sharing with teammates or pasting into onboarding docs.

## Automatic Integration

Other skills should consult `.dev/learnings.jsonl` before making recommendations. When a learning is applied, note: "Prior learning applied: [text]" in the output.

## /gal wrap-up Integration

On `/gal wrap-up`, high-confidence learnings (9+) from the current session are surfaced and offered for inclusion in the active plan's `### Handoff Notes` section.

Tell the user: number of learnings in store, and the top 3 by confidence if viewing all.
