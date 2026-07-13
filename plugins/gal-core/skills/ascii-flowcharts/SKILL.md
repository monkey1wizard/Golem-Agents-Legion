---
name: ascii-flowcharts
description: Explain branching logic, pipelines, algorithms, and multi-step processes as monospaced ASCII decision-tree flowcharts that paste cleanly into PRs, issues, docs, and terminals. Use this whenever you describe how data flows through a multi-stage process, trace per-record control flow, document a pipeline or algorithm with conditions and outcomes, or the user asks for a 流程圖 / 邏輯圖 / flowchart / decision tree — even if they don't name the format explicitly. Prefer this over prose when a process has more than one branch.
---

# ASCII Decision-Tree Flowcharts

## Why this exists

A branching process is far faster to grasp as a picture than as prose. One
record enters at the top, falls through conditions, and lands on an outcome —
the reader sees every path and every dead end at a glance. ASCII keeps that
picture in plain text, so it survives in a PR comment, a code comment, a
Markdown doc, or a terminal with zero tooling and no rendering step.

Reach for this skill whenever a process has **more than one branch**. If the
flow is a straight line with no decisions, a bullet list is fine — don't force
a diagram onto something that isn't branching.

## The core idea: per-record view

Draw the journey of **one item** (one row, one request, one address) from
entry to outcome. Top = input. Each `{ condition? }` splits the path. Every
leaf is a terminal outcome with a graded end-state. This "follow one record
down" framing is what makes the diagram click — the reader mentally drops a
single item in the top and watches where it falls.

## Glyphs

Keep the vocabulary small and consistent — consistency is what makes these
scannable across a whole document.

```
(start) <label>        entry point / the record arriving
[ action ]             a process step — what gets done
{ condition? }         a decision — always end with "?"
  |   v                vertical flow (down)
  +-- <label> -->      a labelled branch off a decision
  ~~>                  async / deferred edge (work handed to a later stage)
  @module.func()       annotate which function/tool performs a step
(end ✅) <outcome>      terminal: normal success / the wanted output
(end ⏭) <outcome>      terminal: skipped on purpose (not an error)
(end ⏹) <outcome>      terminal: dead end / rejected / unresolvable
```

### End-state grading is the point

The three end markers are the highest-value convention here — they let a
reader instantly tell apart "this path produced the real output" (✅), "this
path was deliberately set aside" (⏭), and "this path hit a wall" (⏹). Always
grade every leaf. A diagram where every outcome looks the same wastes the
format.

## Rules that keep them readable

- **Monospace, width ≤ 80 chars.** These live in fixed-width contexts; wrapping
  destroys alignment. Keep lines short enough to never wrap in a GitHub diff.
- **Label every branch.** A bare fork with no `yes`/`no`/value tells the reader
  nothing. Name the condition's outcomes.
- **Annotate the work, not just the shape.** When a specific function, tool, or
  query does the step, tag it with `@name` so the diagram doubles as a map into
  the code. `@tx_date.roc_to_tx_yyyymm`, `@gh pr checks`, `@_dedup_prefix()`.
- **Converge multi-branch downward**, don't sprawl sideways. For a decision with
  many outcomes, list branches with `+--` rather than drawing wide side-by-side
  boxes — it stays within 80 cols and reads top-to-bottom.
- **One diagram, one process.** If a step is itself complex (e.g. a shared key
  builder), give it its own small diagram and reference it by name rather than
  inlining the whole thing.

## Template — decision tree (the common case)

```
(start) <one record arriving>
  |  <key fields it carries>  @<helper that derived them>
  v
{ <first condition>? }
  +-- <value A> --> (end ✅) <output>
  +-- <value B> --> (end ⏭) <why skipped>
  +-- <value C> --> [ <next action> ]  @<func>
                       |
                       v
                     { <next condition>? }
                       +-- yes --> (end ⏹) <dead end / why>
                       +-- no  --> (end ✅) <output>
```

## Template — linear pipeline (few/no branches)

When a step is a straight sequence of transforms with one or two exits, stack
`[ ]` boxes vertically — this is where the format is cleanest:

```
(start) <input>
  |
[ step 1 ]   @func1   <one-line note>
  |
[ step 2 ]   @func2   <one-line note>
  |
{ guard? }
  +-- fail --> (end ⏹) <rejected>
  |
  ok
  v
[ step 3 ]   @func3
  |
(end ✅) <result>
```

## Worked example

A request-handling flow, drawn the way this skill intends:

```
(start) incoming HTTP request
  |  method, path, auth header  @router.match()
  v
{ authenticated? }  @auth.verify_token()
  +-- no --> (end ⏹) 401 rejected
  |
  yes
  v
{ route found? }
  +-- no --> (end ⏹) 404 not found
  +-- yes --> [ handler(req) ]  @dispatch()
                 |
                 v
               { cache hit? }  @cache.get()
                 +-- yes --> (end ✅) 200 from cache
                 +-- no  --> [ query db ]  @db.fetch()  ~~> warm cache
                               |
                               v
                             (end ✅) 200 fresh
```

Read it as one request falling from the top: rejected paths land on `⏹`, both
success paths land on `✅`, and the async cache-warm is a `~~>` side effect that
doesn't block the outcome.

## When NOT to use

- A truly linear narrative with no decisions → use prose or a numbered list.
- A data *structure* (nesting, schema) rather than a *process* → use an indented
  tree or a table.
- A system *architecture* (boxes-and-wires between services) rather than
  per-record flow → that's a different diagram; this skill is about control flow.

---

*Conventions for `(start)`/`[ ]`/`{ }`/`+--`/`~~>` are derived from
tjboudreaux/cc-plugin-text-visualizations (MIT). The per-record framing and the
`✅`/`⏭`/`⏹` graded end-states are GAL additions. See LICENSE.txt.*
