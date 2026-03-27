---
name: obsidian-knowledge-management
description: Knowledge management protocol for the Obsidian vault. PARA + Zettelkasten hybrid system covering note creation, atomization, sanitization, naming, frontmatter, and formatting rules. Use when creating, editing, organizing, or archiving any Obsidian note, processing inbox items, atomizing literature into permanent notes, updating Maps of Content, applying frontmatter templates, naming files with type prefixes, or managing cross-references between vault and codebase.
---

# Obsidian Knowledge Management Protocol

> **Source of Truth**: `99_System/Guide.md` in the <OBSIDIAN_VAULT_NAME>.
> Before any write operation, always load the actual `Guide.md` and relevant `Template_*.md` for the most up-to-date rules. This skill is a quick-reference distillation.

## Step 0: Check Obsidian CLI Availability

Before any vault operation, run the availability check from the `obsidian-cli` skill:

```bash
obsidian vault="<OBSIDIAN_VAULT_NAME>" tags total
```

- **CLI available (exit 0)** → Use `obsidian` CLI for all I/O in this session (read, create, append, property:set, backlinks). This keeps Obsidian's index and graph in sync.
- **CLI unavailable (non-zero / Obsidian closed)** → Fall back to Copilot file tools (`read_file`, `create_file`, `replace_string_in_file`) for the entire session. Follow the fallback message protocol defined in the `obsidian-cli` skill — print intent and ask user to open Obsidian if a CLI-only operation is required.

> The result of this check applies for the **entire conversation session**. Do not re-check on every command.

## Prerequisite: Semantic Search (MANDATORY)

**Before ANY vault operation** (reading, writing, creating, or answering questions from the vault), you MUST invoke the `local-first-search` skill and execute Phase 0 — Semantic Search using `obsidian-note-taking-assistant`:

```bash
cd <LOCAL_SEARCH_PROJECT>
uv run python scripts/query.py semantic "<extracted keywords>" --limit 5
```

- This step is **not optional** and must run before any file-based search or write.
- If results return a similarity score > 0.55, read those notes before proceeding.
- Only after completing this semantic search may you proceed to Context Loading below.
- This applies equally to read-only queries, note creation, inbox processing, and answering conceptual questions.

## Context Loading (Mandatory)

Before ANY file creation, modification, or organization, you MUST read:

1. `99_System/Guide.md` (Workflow, Naming, Formatting, Frontmatter)
2. `99_System/Tag_Taxonomy.md` (Valid tags)
3. `99_System/User_Context_Profile.md` (User persona)
4. The specific `99_System/Template_*.md` required by the task

## Agent Mandates

- **No Over-interpretation**: Execute ONLY what is explicitly requested.
- **Strict File Operation Protocol**: Only operate on files mentioned by the user. For any other file, explain Which/Why/How and wait for approval.
- **Pre-Write Validation**: Verify output against `Guide.md` before writing.
- **Strict Terminology (Taiwanese/English ONLY)**: You MUST strictly avoid using Mainland Chinese terminology (e.g., 網絡端, 內核, 緩存, 虛擬機, 軟件、進程). You MUST use Taiwanese terminology (e.g., 網頁端/前端, 核心, 快取, 虛擬機器, 軟體, 程序/常駐程式) or fallback to English if unsure.
- **Link Integrity**: When renaming a file, search all backlinks (`[[OldName]]`) and update them to `[[NewName]]`.
- **Strict Batch Protocol**: When processing multiple files:
  1. List ALL target files first.
  2. Process ONE file per response.
  3. STOP and wait for user approval before the next.

## Available Scripts / Tools

When you encounter a PDF file in the Vault, follow the **pdf** skill for extraction.

## Vault Structure (PARA + Zettelkasten)

```text
00_Inbox/          # Raw input. Goal: empty this (except 000_Linklist.md)
10_Projects/       # Active projects, specs, logs
20_Slipbox/        # Core brain
  21_Literature/   # Long-term literature (only if user explicitly requests)
  22_Permanent/    # Atomic notes (Concepts, Models, Strategies...)
  23_Maps/         # MAP (Map of Content) index notes
30_Archives/       # Read-only. Processed literature & outdated records
99_System/         # Templates, taxonomy, guides
```

### Data Validity Boundary

- **Valid**: Only folders matching `NN_Title` pattern (e.g., `00_Inbox`, `10_Projects`).
- **Invalid/Legacy**: Folders like `!Flash Idea`, `!Logs`, `Permanent` — **ignore, no read, no use**.

### Data Flow

```text
[ External Sources ]
YouTube / Articles / News
     │
     ▼ (1. Collect & Sanitize)
[ 00_Inbox / 30_Archives ]
Clips_Text / Clips_Media
     │
     ├─► (2a. Extract Universal Knowledge)
     │       │
     │       ▼
     │   [ 22_Permanent ]
     │   Concept_ / Model_ / Strategy_
     │       │
     │       ├─► (3a. Aggregate & Index)
     │       │       ▼
     │       │   [ 23_Maps ]
     │       │   Map_of_Content.md
     │       │
     │       └─► (3b. Provide Decision Logic) ─┐
     │                                         │
     └─► (2b. Personal Market Observations) ───┤
                                               ▼
                                       [ 10_Projects ]
                                       Yearly Plan / Long-term Allocation
                                               │
                                               ▼ (4. Year or Project Ends)
                                       [ 30_Archives ]
                                       Historical Projects & Logs
```

## File Naming Rules

### General

- **Forbidden chars**: `: / \ ? * " < > |` → replace with `_` or `-`.
- **Spaces** → underscore `_` (Snake_Case).
- **Brackets** `《》[]()` → remove.

### By Directory

| Directory | Format | Example |
| --- | --- | --- |
| `21_Literature` | `YYYYMMDD_Author_Short_Title.md` | `20251208_Lailari_China_Strategy.md` |
| `22_Permanent` | `Type_Keyword.md` | `Concept_拒止戰略_Strategy_of_Denial.md` |
| `23_Maps` | `Topic_Name.md` | `人工智慧.md` |

### Permanent Note Type Prefixes

| Type | Purpose |
| --- | --- |
| `Concept` | "What" — definitions, principles, phenomena |
| `Strategy` | "How" — action plans, methodologies |
| `Model` | "Mechanism" — causal, input→output structures |
| `Pattern` | "Recurring solution/phenomenon" — empirical, repeatable |
| `Case` | Specific real-world instance or evidence |
| `Data` | Datasets, statistics, reference data |
| `Resource` | Curated resource lists |
| `Tool` | Software, services, utilities |
| `Work` | Creative works, projects |

### Atomicity Principles

- **Universal concept** (reusable across contexts) → **must split** into its own file.
- **Exception (strong coupling)**: If a concept only exists in one case and has no standalone value, merge into one file using the dominant type as prefix.

## Frontmatter Standards

### Permanent Note (`Template_Permanent.md`)

```yaml
---
source: "[[Source_Literature_Note]]"
type: concept          # concept | model | strategy | tool | pattern | case | data | resource | work
updated: "YYYY-MM-DD"
tags:
  - domain-tag
  - specific-tag
---
```

### Literature — Text (`Template_Clips_Text.md`)

```yaml
---
source: {{url}}
author: {{author}}        # Plain text, NO wiki-links
type: literature
published: {{date}}
tags:
  - domain-tag
  - specific-tag
---
```

### Literature — Media (`Template_Clips_Media.md`)

```yaml
---
source: {{url}}
author: {{channel_name}}   # Plain text, NO wiki-links
type: literature
published: {{date}}
tags:
  - domain-tag
  - specific-tag
---
```

### MAP (`Template_Map.md`)

```yaml
---
type: map
updated: {{date}}
tags:
  - map-of-content
  - domain-tag
---
```

### Key Frontmatter Rules

- `source`: Use Wiki Link `[[Note]]` if source exists in vault; URL if external; `ai-synthesis` only for AI-generated content with no specific vault source; `personal-observation` for personal data.
- `author`: **Plain text only**. No wiki-links.
- `tags`: **English (en-US)**, **kebab-case** (e.g., `#knowledge-management`).
- Template `<!-- ... -->` comments are meta-instructions: follow them, then **remove** from final output.

## Workflow: Inbox Processing

### Triage Logic (Decision Tree)

1. **`000_Linklist.md`**: Process its content but **never move/delete** this file.
2. **History** (meeting notes, expired events) → move to `30_Archives/`.
3. **Knowledge Source** (articles, videos, papers):
   a. **Sanitize** (see below).
   b. **Apply Template** (Text or Media).
   c. **Atomize** (extract permanent notes).
   d. **Archive**: Default → `30_Archives/`. Only to `21_Literature/` if user explicitly requests.

### Content Sanitization (Non-Destructive)

**Remove ONLY** (never modify existing text):

- Banner ads, CTAs ("Subscribe", "Donate"), sidebar widgets.
- Auto-generated "Related articles" (unless highly relevant).
- Header (logo, nav menus, search bar — anything before H1).
- Footer (copyright, sitemap, privacy policy).
- Author self-promotion (keep author name only).
- Decorative images (keep diagrams, charts, data visualizations).

**Preserve**:

- Author disclosures, sponsor mentions relevant to context.
- All knowledge-bearing images (architecture diagrams, flowcharts, data charts).

### Atomization Process

1. **Identify**: Read source, ignore noise.
2. **Extract**: Find core concepts, universal models, concrete cases.
3. **Transform**: Create independent `.md` files in `22_Permanent/` using `Type_Keyword.md` naming.
4. **Bi-directional Linking**:
   - Permanent → Literature: include `source: [[Literature_Note]]` in frontmatter.
   - Literature → Permanent: **do not edit** source text; rely on Backlinks. Exception: append `參考: [[Related]]` at end for closely related sources only.
5. **Archive**: Move literature to `30_Archives/`. Content must remain **verbatim** — no summarization.

### Auto-Tagging

1. Analyze content semantically → assign 3–5 tags (English, kebab-case).
2. Write to frontmatter `tags` field.
3. Check `99_System/Tag_Taxonomy.md` — if a new tag is needed, add its definition to the taxonomy.

### MAP Maintenance

- **Trigger**: 3+ notes in `22_Permanent/` share a topic → create/update a MAP in `23_Maps/`.
- **Structure**: Knowledge Clusters (permanent notes), Key Literature, Related Projects.
- **Proactive**: After creating any `22_Permanent` note, search existing MAPs and add the new note if relevant.

## Formatting & Style

### Link Format

- **Always** use Wiki Links `[[File_Name]]`. Never use Markdown links `[title](path)` for internal notes.

### Language

- Match source language (en/ja/zh).
- Default Chinese output: **Traditional Chinese (zh-Hant/zh-TW)**. Never mix Simplified and Traditional.
- **Strictly use Taiwanese Terminology** (e.g., 核心, 快取, 虛擬機器, 軟體). DO NOT use Mainland Chinese terminology (e.g., 內核, 緩存, 虛擬機, 軟件).
- Keep English terms when no precise zh-Hant equivalent exists.

### Punctuation

- **CJK text**: Use full-width symbols `：`、`（`、`）`、`，`、`。`.
- **No semicolons** (`;` or `；`). Use commas or periods instead.
- **Half-width only for**: English terms in CJK text `(example)`, full English sentences, code blocks, URLs.

### Content Style

- **No Emoji** (unless for warnings).
- **Quality over quantity** for contextual links — only strong connections.
- **Link validation**: Every `[[link]]` must point to an existing file; create the file or omit the link.
- **Hierarchy over lists**: Use heading levels (`###`, `####`) for long content instead of flat bullet lists.
- **Flexible numbering**: Sequential (`1.`/`2.`), categorical (`A.`/`B.`), or CJK (`一、`/`二、`) — but consistent within the same level.

### References Section

Every note should end with:

```markdown
## 參考資料 (References)

- [External Title](URL)
- [[Internal_Note]]
```

## Note Structure Quick Reference

### Permanent Note

```markdown
# 中文標題 (English Title)

Brief definition/background.

## 詳細內容 (Content)

### 核心定義 / 運作機制 (Core Definition / Mechanism)
...

### 執行步驟 / 具體作法 (Execution / Tactics)
#### 1.
...

---
## 脈絡連結 (Contextual Links)

- **上層概念 (Parent)**：[[Parent_Note]]
- **相關概念 (Related)**：[[Related_Note]]
- **應用/案例 (Applications)**：[[Case_Note]]
- **相關索引 (Map)**：[[Map_Note]]

## 參考資料 (References)
```

### MAP Note

```markdown
# MAP：Topic_Name

## 關聯地圖 (Related Maps)
- 上層地圖：[[Parent_Map]]
- 子地圖：[[Sub_Map]]

## 知識聚落 (Knowledge Clusters)
- [[Permanent_Note]]: Brief context.

## 關鍵文獻 (Key Literature)
- [[Literature_Note]]：Why it matters.

## 相關專案 (Related Projects)
- [[Project_Note]]：Status/relevance.
```
