---
name: local-first-search
description: Search the existing <OBSIDIAN_VAULT_NAME> for knowledge using Local-First and Context-First retrieval. If a personal Guide exists, follow its structure first; otherwise default to a PARA-based search order. Activate before answering conceptual questions, proposing solutions, or creating new notes.
---

# Local-First Search Strategy

## Core Philosophy

**Context First, Local First**: Before answering questions, proposing solutions, or creating new notes, you must search the user's <OBSIDIAN_VAULT_NAME> for existing knowledge first. Avoid generic answers that ignore the vault.

**Structure Resolution**:

- If a personal Guide exists, load it and use the Guide-defined note locations, naming rules, and search priorities.
- If no Guide exists, default to a standard PARA system: Projects, Areas, Resources, and Archives.
- Do not assume numbered folder prefixes, Slipbox folders, MOC folders, or private naming conventions unless the Guide or vault clearly shows them.

## Semantic Search Tool (Primary Search Method)

The `obsidian-note-taking-assistant` provides a DuckDB + BGE-M3 vector search engine over the entire vault. Use this before any file-based search.

### Tool Location

- **Project**: `<LOCAL_SEARCH_PROJECT>`
- **Script**: `scripts/query.py`
- **Run via**: `uv run python scripts/query.py <command> <args>`

### Available Query Commands

| Command | Use Case | Example |
| --- | --- | --- |
| `semantic "<query>" --limit N` | General conceptual search | `semantic "data contracts" --limit 5` |
| `graph-boosted "<query>" --seed "<slug>" --boost N` | Search amplified by wikilink graph proximity to a known note | `graph-boosted "api design" --seed "resources-concept_data_contracts" --boost 1.3` |
| `backlinks "<slug>"` | Get all notes that link to a specific note | `backlinks "resources-concept_data_contracts"` |
| `connections "<slug>" --hops N` | Traverse the wikilink graph N hops out | `connections "resources-concept_data_contracts" --hops 2` |
| `shared-tags "<slug>" --min-shared N` | Find notes sharing at least N tags with a given note | `shared-tags "resources-concept_data_contracts" --min-shared 2` |
| `sql "<SQL>"` | Raw SQL query against DuckDB | `sql "SELECT title, slug FROM notes LIMIT 10"` |

### Slug Format

Slugs are auto-generated from file paths. Pattern: lowercase path with `/` turned into `-`, and spaces turned into `-`.

- `Resources/Concept_Data_Contracts.md` becomes `resources-concept_data_contracts`
- Use the `slug` field returned in search results to compose `graph-boosted` or `backlinks` queries.

### Re-indexing

Run when the vault has new notes:

```bash
cd <LOCAL_SEARCH_PROJECT>
uv run python scripts/ingest.py "<OBSIDIAN_VAULT>" --model "BAAI/bge-m3"
```

> First run downloads the BGE-M3 model. Subsequent runs use cache.

## Target Vault Path

- **Vault Path**: `<OBSIDIAN_VAULT>`

## Search Priorities

When executing a search, use this order:

1. **Guide-defined reference or knowledge locations**
   - If the user's Guide defines specific reference folders, knowledge folders, or indexes, search those first.

2. **Resources (default PARA highest priority)**
   - Use Resources for reusable knowledge, cheatsheets, references, glossaries, literature summaries, and long-lived learning material.
   - If a query looks like a direct lookup request, Resources should usually be the first PARA category searched.

3. **Projects**
   - Search Projects when the question is about active work, plans, logs, or deliverables.

4. **Areas**
   - Search Areas when the question concerns ongoing responsibilities, maintained domains, or repeated operational context.

5. **Archives**
   - Search Archives only when the first four steps do not produce relevant results, or when the user explicitly wants historical material.

6. **Optional inbox or capture folders**
   - Search them only when the user asks about unprocessed captures, drafts, or raw intake material.

## Excluded Directories

Ignore these by default unless the user explicitly asks for them:

- Hidden, temp, log, export, or cache folders
- Legacy folders that the Guide marks as deprecated
- Loose root files that are clearly operational noise rather than user knowledge

## Execution Workflow: Answering Questions

When you need to retrieve information to answer a user's question:

### Phase 0 — Semantic Search (Always First)

Run the vector search tool before any file-based search:

```bash
cd <LOCAL_SEARCH_PROJECT>
uv run python scripts/query.py semantic "<extracted keywords>" --limit 5
```

- If a highly relevant note appears in results, read it via the slug.
- If you know a good seed note from the results, escalate to `graph-boosted` for richer context.
- If Phase 0 returns at least one high-confidence result, skip the later directory-first fallbacks and go directly to reading files.

### Phase 1 — Filename Search

If vector search yields low scores, prioritize filename search in:

- Guide-defined knowledge or reference locations, if present
- Resources by default
- Projects or Areas when the question is clearly scoped there

### Phase 2 — Full-Text Search

If filename search fails, use full-text search.

### Phase 3 — Read & Synthesize

1. **Read context**: Use the file reading tool to read full notes, not just snippets.
2. **Synthesize & cite**:
   - Responses must be primarily based on vault content.
   - Whenever you use vault knowledge, cite the source note using wiki-link format such as `[[Note Name]]`.
   - If you supplement with non-vault knowledge, say so explicitly.

## Execution Workflow: Pre-Write Search

When you are about to add new knowledge, restructure notes, or create new notes in the vault:

1. **Check for duplicates**
   - Extract the core concept or target outcome.
   - Search the Guide-defined destination first, or the relevant PARA category if no Guide exists.

2. **Merge vs. create**
   - If a matching note already exists, update or merge instead of duplicating.
   - If no note exists, create a new note using the Guide's naming rules, or a clear PARA-compatible name by default.

3. **Indexes or maps**
   - If the user's Guide defines maps, dashboards, indexes, or MOCs, update them when relevant.
   - If no such structure exists, do not force index maintenance.

4. **Project document updates**
   - When updating project notes or logs, search the Guide-defined project location first, or Projects by default.
   - Project planning for the knowledge base belongs in the vault's project area, not inside the code "<RESEARCH_DEFAULT_DEST>"sitory unless the user explicitly wants "<RESEARCH_DEFAULT_DEST>" docs.

## Cross-Boundary Referencing (Vault vs. Codebase)

Because the Obsidian vault and code "<RESEARCH_DEFAULT_DEST>"sitories are separate systems, respect the boundary between them:

- **When writing in a code "<RESEARCH_DEFAULT_DEST>"sitory**: Do not use Obsidian wiki links to reference vault notes. Use plain text descriptions or standard file links instead.
- **When writing in the Obsidian vault**: Use wiki links for internal vault notes. Use standard Markdown links or plain text paths for external code-"<RESEARCH_DEFAULT_DEST>"sitory files.
