---
name: local-first-search
description: Search the existing Obsidian Vault for knowledge, prioritizing the 20_Slipbox directory. Use before answering questions, proposing solutions, or creating new notes to adhere to the Local-First and Context-First philosophy. Activate when the user asks a conceptual question, requests information that may exist in their vault, wants to look up a cheatsheet or reference table, or needs to verify existing knowledge before creating new content.
---

# Local-First Search Strategy

## Core Philosophy

**Context First, Local First**: Before answering questions, proposing solutions, or creating new notes, you **MUST** first search the user's Obsidian Vault for existing knowledge, models, and concepts. Avoid generating purely generic AI responses; your answers must be grounded in the contents of the user's curated Vault.

## Semantic Search Tool (Primary Search Method)

The **`obsidian-note-taking-assistant`** provides a DuckDB + BGE-M3 vector search engine over the entire Vault. Use this **before** any file-based search.

### Tool Location
- **Project**: `C:\Users\leetz\obsidian-note-taking-assistant`
- **Script**: `scripts/query.py`
- **Run via**: `uv run python scripts/query.py <command> <args>`

### Available Query Commands

| Command | Use Case | Example |
| --- | --- | --- |
| `semantic "<query>" --limit N` | General conceptual search | `semantic "Zettelkasten 知識管理" --limit 5` |
| `graph-boosted "<query>" --seed "<slug>" --boost N` | Search amplified by Wikilink graph proximity to a known note | `graph-boosted "知識管理" --seed "20_slipbox-22_permanent-concept_xxx" --boost 1.3` |
| `backlinks "<slug>"` | Get all notes that link to a specific note | `backlinks "concept_zettelkasten"` |
| `connections "<slug>" --hops N` | Traverse the Wikilink graph N hops out | `connections "data-contracts" --hops 2` |
| `shared-tags "<slug>" --min-shared N` | Find notes sharing ≥N tags with a given note | `shared-tags "data-contracts" --min-shared 2` |
| `sql "<SQL>"` | Raw SQL query against DuckDB | `sql "SELECT title, slug FROM notes LIMIT 10"` |

### Slug Format
Slugs are auto-generated from file paths. Pattern: lowercase path with `/` → `-` and spaces → `-`.
- `20_Slipbox/22_Permanent/Concept_知識管理.md` → slug: `20_slipbox-22_permanent-concept_知識管理`
- Use the `slug` field returned in search results to compose `graph-boosted` / `backlinks` queries.

### Re-indexing (run when Vault has new notes)
```bash
cd C:\Users\leetz\obsidian-note-taking-assistant
uv run python scripts/ingest.py "C:\Users\leetz\OneDrive\Obsidian Vault" --model "BAAI/bge-m3"
```
> Note: First run downloads BGE-M3 model (~1GB). Subsequent runs use cache and take ~35 minutes.

---

## Target Vault Path

- **Vault Path**: `c:\Users\leetz\OneDrive\Obsidian Vault`

## Search Priorities

When executing a search, strictly follow this hierarchy of directories and note types:

1. **`20_Slipbox/22_Permanent/Resource_*` (Highest Priority — Cheatsheets & Reference Tables)**
   - **Definition**: Resource-type notes (`Resource_*.md`) are curated cheatsheets, reference tables, and speed-lookup guides. They contain ready-to-use, structured reference data (e.g., API type comparisons, Git command cheatsheets, .NET code hacks).
   - **Action**: When a query matches a domain covered by a Resource note, **prioritize it above all other note types**. These notes are designed for direct lookup and should be the first result presented. Search by filename pattern `Resource_*keyword*`.
   - **Current Resource Notes**:
     - `Resource_API_Types_Cheatsheet.md` — API type comparison & selection
     - `Resource_CSharp_DotNET_速查表.md` — C#/.NET code hacks
     - `Resource_Git_進階指令速查表.md` — Advanced Git commands

2. **`20_Slipbox/22_Permanent/` (High Priority — Other Atomic Notes)**
   - **Definition**: Contains the core "atomic notes" (Concepts, Models, Strategies, Patterns, Cases, Tools, etc.). This is the most refined and directly applicable knowledge.
   - **Action**: Search here after checking for matching Resource notes. If relevant notes are found, read them in detail and use them as the primary basis for your answer.

3. **`20_Slipbox/23_Maps/`**
   - **Definition**: Contains Maps of Content (MOCs) and topic indices.
   - **Action**: Search here if a single keyword search fails to find specific atomic notes, or to understand the broader context of a topic.

4. **`20_Slipbox/21_Literature/`**
   - **Definition**: High-value literature notes retained long-term.
   - **Action**: Search here specifically if the user asks about particular authors, books, articles, or external reference sources.

5. **`10_Projects/`**
   - **Definition**: Active projects, planning documents, and logs.
   - **Action**: Search here when the query relates to annual goals, investment plans, or specific project statuses.

6. **`30_Archives/` (Lowest Priority)**
   - **Definition**: Archived literature and historical records.
   - **Action**: Perform text searches here *only* if no relevant information is found in `20_Slipbox`.

## Excluded Directories

**ABSOLUTELY DO NOT** read or use contents from the following directories (unless explicitly requested):

- `!Flash Idea/`
- `!Logs/`
- `Permanent/` (Legacy structure, distinct from `22_Permanent`)
- Loose files in the root of the Vault.

## Execution Workflow: Answering Questions

When you need to retrieve information to answer a user's question:

### Phase 0 — Semantic Search (ALWAYS FIRST)
Run the vector search tool before any file-based operation:

```bash
cd C:\Users\leetz\obsidian-note-taking-assistant
uv run python scripts/query.py semantic "<extracted keywords>" --limit 5
```

- If a highly relevant note appears in results (similarity > 0.55), use `run_in_terminal` to read it via the slug.
- If you know a related seed note from the results, escalate to `graph-boosted` for richer context:
  ```bash
  uv run python scripts/query.py graph-boosted "<query>" --seed "<slug from results>" --boost 1.3
  ```
- If Phase 0 returns ≥1 high-confidence result, **skip Phase 1 & 2** and go directly to reading the files.

### Phase 1 — Filename Search (fallback if vector search yields low scores)
Prioritize searching by filename (e.g., `*keyword*.md`) within `22_Permanent/` and `23_Maps/`.

### Phase 2 — Full-Text Search (fallback if Phase 1 fails)
If filename search fails, use a full-text search (e.g., `grep_search`).

### Phase 3 — Read & Synthesize (MANDATORY)
1. **Read Context**: You **MUST** use the file reading tool to read the complete content of found notes. Do not guess based on text snippets.
2. **Synthesize & Cite (MANDATORY)**:
   - Your response must be **primarily based** on the Vault content.
   - **CRITICAL**: Whenever you use knowledge from the vault, you **MUST explicitly cite the source article** using Wiki Link format (e.g., `[[Note Name]]`).
   - If supplementing with external AI knowledge, explicitly state: *"The Vault does not mention this aspect; the following is supplementary..."*

## Execution Workflow: Pre-Write & Database Update

When you are tasked with adding new knowledge, restructuring, or creating notes in the Vault, you MUST perform a "Pre-Write Search" to prevent duplication and maintain the Knowledge Network:

1. **Check for Duplicates (Pre-Write Search)**:
   - Before creating any new atomic note in `22_Permanent/`, extract the core concept/keyword.
   - Search the `22_Permanent/` directory to see if a note covering this concept already exists.
2. **Merge vs. Create**:
   - **If it exists**: DO NOT create a duplicate note. Read the existing note and **update/merge** the new information into the existing file.
   - **If it does not exist**: Create the new atomic note following the standard naming and formatting rules (`Type_Keyword.md`).
3. **Map Connection (Post-Write Search)**:
   - After creating or updating a permanent note, you must determine its broader topic cluster.
   - Search the `23_Maps/` directory to find an existing relevant Map of Content (MOC).
   - If a relevant MOC is found, **update the MOC** to include a `[[backlink]]` to the newly updated/created permanent note, ensuring there are no orphaned notes in the vault.
4. **Project Document Updates**:
   - When asked to update a project's notes or logs, you must first search the `10_Projects/` directory in the Obsidian Vault to find the corresponding project file (e.g., `10_Projects/Project_Name.md` or a sub-folder).
   - Project documentation and planning for the knowledge base should reside in `10_Projects/` following Obsidian standards, not directly inside the completely separate codebase repository.

## Cross-Boundary Referencing (Vault vs. Codebase)

Because the Obsidian Vault and your coding repositories reside in completely different directories and use different version control systems (e.g., OneDrive vs. Git), you must respect the boundary between them:

- **When writing in a Code Repository** (e.g., creating project plans like `docs/plans/` or updating READMEs): **DO NOT** use Obsidian Wiki Links (`[[Note Name]]`) to reference concepts from the vault. Code repositories cannot resolve Obsidian's logical links. Use plain text descriptions or standard absolute paths instead.
- **When writing in the Obsidian Vault**: Continue using standard Wiki Links (`[[...]]`) for internal vault references. If you need to reference a file from a code repository, use standard Markdown file links (e.g., `[file](/path/to/repo/file)`) or plain text paths, as Obsidian cannot logically track external Git files.
