# graphify Pipeline Execution Guide

Complete documentation for executing graphify knowledge graph extraction on a codebase, including workarounds for API version differences.

## Overview

graphify is a knowledge graph extraction tool that converts codebases into structured graph representations. This guide documents the complete pipeline execution process, including critical API compatibility issues between documented version (0.4.23) and current version (0.5.7+).

**Pipeline Output:**
- Interactive HTML visualization (D3.js force-directed graph)
- Structured JSON graph data with community detection
- Markdown analysis report with god nodes, surprising connections, and suggested questions
- Token compression benchmarks

## Prerequisites

### Environment Setup

```powershell
# Windows PowerShell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy RemoteSigned
cd C:\Code\Golem-Agents-Legion
.\.venv\Scripts\Activate.ps1

# Verify graphify installation
graphify --version
```

### Required Python Packages

```bash
pip install graphify>=0.5.7
```

### Directory Structure

```
your-repo/
  graphify-out/          # Output directory (auto-created)
    .graphify_python     # Cached Python interpreter path
    .graphify_detect.json
    .graphify_ast.json
    .graphify_semantic.json
    .graphify_extract.json
    .graphify_analysis.json
    .graphify_labels.json
    graph.json
    graph.html
    GRAPH_REPORT.md
    .graphify_benchmark.json
```

## CRITICAL: API Version Differences

The graphify SKILL.md documentation was written for version 0.4.23. **Version 0.5.7+ has completely different function signatures.** Do NOT follow the SKILL.md API examples directly.

### Key API Changes (0.4.23 → 0.5.7+)

| Function | Old API (0.4.23) | New API (0.5.7+) |
|----------|------------------|------------------|
| `cluster()` | Returns `{node_id: community_id}` | Returns `{community_id: [node_ids]}` |
| `god_nodes()` | `(G, communities, top_n=10)` | `(G, top_n=10)` - NO communities param |
| `surprising_connections()` | Different signature | `(G, communities, top_n=5)` - requires dict format |
| `suggest_questions()` | `(G, communities, community_labels=None, top_n=7)` | **REQUIRES** `community_labels` dict - cannot be None |
| `generate()` | ~6 parameters | 10 parameters including `suggested_questions` list |
| `to_html()` | Different signature | `(G, output_path, communities, community_labels=None)` |

### Critical Fix: community_labels

**NEVER pass `None` to `suggest_questions()`**. The function will fail at line 377 with `AttributeError: 'NoneType' object has no attribute 'get'`.

**Always provide a dict:**
```python
# Minimum working version (temporary labels)
temp_labels = {cid: f"Community {cid}" for cid in community_lists.keys()}
questions = suggest_questions(G, community_lists, community_labels=temp_labels, top_n=7)

# Better version (semantic labels)
labels_data = json.loads(Path('graphify-out/.graphify_labels.json').read_text())
questions = suggest_questions(G, community_lists, community_labels=labels_data, top_n=7)
```

## Pipeline Execution Steps

### Step 0: Cache Python Interpreter

**Purpose:** Ensure consistent Python environment across all pipeline steps.

```powershell
$PYTHON = (Get-Command python).Source
$PYTHON | Out-File -FilePath "graphify-out/.graphify_python" -Encoding utf8 -NoNewline
Write-Host "Python interpreter cached: $PYTHON"
```

**Output:** `graphify-out/.graphify_python`

---

### Step 1: Verify Installation

**Purpose:** Check graphify is installed and accessible.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c "import graphify; print(f'graphify {graphify.__version__}')"
```

**Expected Output:** `graphify 0.5.7` (or higher)

---

### Step 2: File Detection

**Purpose:** Scan repository and categorize files (code, documents, papers, images).

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
from graphify.detect import detect_corpus
from pathlib import Path
import json

result = detect_corpus(Path('.'), output_dir=Path('graphify-out'))
print(f'Files: {result[\"total_files\"]}')
print(f'Words: {result[\"total_words\"]:,}')
print(f'Code: {len(result[\"files\"][\"code\"])}')
print(f'Docs: {len(result[\"files\"][\"document\"])}')
"@
```

**Output:** `graphify-out/.graphify_detect.json`

**Sample Data:**
```json
{
  "files": {
    "code": ["file1.ps1", "file2.py"],
    "document": ["doc1.md", "doc2.html"],
    "paper": [],
    "image": [],
    "video": []
  },
  "total_files": 166,
  "total_words": 161513,
  "needs_graph": true,
  "warning": null
}
```

---

### Step 3A: AST Extraction

**Purpose:** Extract Abstract Syntax Tree nodes and edges from code files.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from graphify.extract import collect_files, extract
from pathlib import Path

detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text())
code_files = []
for f in detect.get('files', {}).get('code', []):
    code_files.extend(collect_files(Path(f)) if Path(f).is_dir() else [Path(f)])

if code_files:
    result = extract(code_files)
    Path('graphify-out/.graphify_ast.json').write_text(json.dumps(result, indent=2))
    print(f'AST: {len(result[\"nodes\"])} nodes, {len(result[\"edges\"])} edges')
else:
    empty = {'nodes':[], 'edges':[], 'input_tokens':0, 'output_tokens':0}
    Path('graphify-out/.graphify_ast.json').write_text(json.dumps(empty))
    print('No code files - skipping AST extraction')
"@
```

**Output:** `graphify-out/.graphify_ast.json`

---

### Step 3B: Semantic Extraction (Parallel)

**Purpose:** Extract semantic nodes and edges from documentation using LLM reasoning.

**IMPORTANT:** Use parallel Explore subagents for large document sets (100+ files). Each subagent processes a chunk independently.

**Chunk Preparation:**
```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text())
doc_files = detect.get('files', {}).get('document', [])
chunk_size = max(1, len(doc_files) // 6)  # 6 parallel chunks

for i in range(6):
    start = i * chunk_size
    end = start + chunk_size if i < 5 else len(doc_files)
    chunk = doc_files[start:end]
    Path(f'graphify-out/.graphify_chunk_{i:02d}.txt').write_text('\n'.join(chunk))
    print(f'Chunk {i}: {len(chunk)} files')
"@
```

**Parallel Execution:**
Run 6 Explore subagents simultaneously, each with:

```
Extract knowledge graph (nodes, edges, hyperedges) from these files:
[paste chunk file list]

Return ONLY valid JSON in this format:
{
  "nodes": [
    {"id": "unique_id", "type": "concept|function|class|file", "label": "display_name", "description": "brief_description"}
  ],
  "edges": [
    {"source": "node_id", "target": "node_id", "type": "calls|imports|extends|uses", "confidence": "DIRECT|INFERRED"}
  ],
  "hyperedges": [
    {"nodes": ["id1", "id2", "id3"], "type": "relationship_type", "description": "what_connects_them"}
  ]
}

Focus on architectural relationships, not implementation details.
```

**Save each subagent result:**
```powershell
# After each subagent completes, save its JSON output
$result = @"
{subagent_json_output}
"@
$result | Out-File -FilePath "graphify-out/.graphify_chunk_00.json" -Encoding utf8
```

**Merge Chunks:**
```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

merged = {'nodes': [], 'edges': [], 'hyperedges': [], 'input_tokens': 0, 'output_tokens': 0}
for i in range(6):
    chunk_file = Path(f'graphify-out/.graphify_chunk_{i:02d}.json')
    if chunk_file.exists():
        chunk = json.loads(chunk_file.read_text())
        merged['nodes'].extend(chunk.get('nodes', []))
        merged['edges'].extend(chunk.get('edges', []))
        merged['hyperedges'].extend(chunk.get('hyperedges', []))

Path('graphify-out/.graphify_semantic.json').write_text(json.dumps(merged, indent=2))
print(f'Merged: {len(merged[\"nodes\"])} nodes, {len(merged[\"edges\"])} edges')
"@
```

**Output:** `graphify-out/.graphify_semantic.json`

---

### Step 3C: Merge AST + Semantic

**Purpose:** Combine code-based and document-based extractions.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

ast = json.loads(Path('graphify-out/.graphify_ast.json').read_text())
semantic = json.loads(Path('graphify-out/.graphify_semantic.json').read_text())

merged = {
    'nodes': ast.get('nodes', []) + semantic.get('nodes', []),
    'edges': ast.get('edges', []) + semantic.get('edges', []),
    'hyperedges': semantic.get('hyperedges', []),
    'input_tokens': 0,
    'output_tokens': 0
}

Path('graphify-out/.graphify_extract.json').write_text(json.dumps(merged, indent=2))
print(f'Total: {len(merged[\"nodes\"])} nodes, {len(merged[\"edges\"])} edges')
"@
```

**Output:** `graphify-out/.graphify_extract.json`

---

### Step 4: Graph Analysis

**Purpose:** Build NetworkX graph, detect communities, calculate metrics, identify god nodes and surprising connections.

**CRITICAL:** This step has the most API compatibility issues. Use this exact implementation:

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json, sys
from pathlib import Path
from graphify.build import build_from_json
from graphify.cluster import cluster
from graphify.analyze import god_nodes, surprising_connections, suggest_questions, cohesion

# Load and build graph
extract_data = json.loads(Path('graphify-out/.graphify_extract.json').read_text())
G = build_from_json(extract_data)
print(f'Graph: {G.number_of_nodes()} nodes, {G.number_of_edges()} edges')

# Community detection - returns {community_id: [node_ids]}
community_lists = cluster(G)
print(f'Communities detected: {len(community_lists)}')

# Calculate cohesion scores
cohesion_scores = {}
for cid, nodes in community_lists.items():
    cohesion_scores[cid] = cohesion(G, nodes)

# God nodes - NO communities parameter
gods = god_nodes(G, top_n=10)

# Surprising connections - requires communities dict
surprises = surprising_connections(G, communities=community_lists, top_n=10)

# Suggested questions - REQUIRES community_labels dict (cannot be None)
# Use temporary labels for now (will regenerate with semantic labels later)
temp_labels = {cid: f'Community {cid}' for cid in community_lists.keys()}
questions = suggest_questions(G, community_lists, community_labels=temp_labels, top_n=7)

# Save analysis results
analysis = {
    'communities': {str(k): v for k, v in community_lists.items()},
    'cohesion': {str(k): v for k, v in cohesion_scores.items()},
    'gods': gods,
    'surprises': surprises,
    'questions': questions
}
Path('graphify-out/.graphify_analysis.json').write_text(json.dumps(analysis, indent=2))

print(f'Analysis complete:')
print(f'  {len(gods)} god nodes')
print(f'  {len(surprises)} surprising connections')
print(f'  {len(questions)} suggested questions')
"@
```

**Output:** `graphify-out/.graphify_analysis.json`

**Sample Data:**
```json
{
  "communities": {
    "0": ["node1", "node2"],
    "1": ["node3", "node4"]
  },
  "cohesion": {
    "0": 0.75,
    "1": 0.82
  },
  "gods": [
    {"node": "MCPConnection", "degree": 11},
    {"node": "run_loop", "degree": 9}
  ],
  "surprises": [
    {
      "source": "Function1",
      "target": "Function2",
      "confidence": "INFERRED",
      "source_file": "file1.ps1",
      "target_file": "file2.ps1"
    }
  ],
  "questions": [...]
}
```

---

### Step 5: Community Labeling

**Purpose:** Generate semantic labels for each community using LLM.

**Prepare Sample Nodes:**
```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())
communities = {int(k): v for k, v in analysis['communities'].items()}

prep_data = {}
for cid, nodes in communities.items():
    prep_data[cid] = nodes[:10]  # First 10 nodes per community

Path('graphify-out/.graphify_prep_labels.json').write_text(json.dumps(prep_data, indent=2))
print(f'Prepared {len(prep_data)} communities for labeling')
"@
```

**Generate Labels with LLM:**

Use an LLM to generate semantic labels for each community. For each community, provide the sample nodes and ask:

```
Community {id} contains these nodes:
{node_list}

Generate a concise 3-5 word label that describes the primary purpose or theme of this community.
Focus on what these components DO, not implementation details.

Return ONLY the label text, no explanation.
```

**Save Labels:**
```powershell
# After getting all labels from LLM
$labels = @{
    "0" = "Setup Utilities & Script Updates"
    "1" = "MCP Configuration Management"
    "2" = "GAL Core State & Plan Management"
    # ... continue for all communities
}

$labels | ConvertTo-Json -Depth 10 | Out-File -FilePath "graphify-out/.graphify_labels.json" -Encoding utf8
```

**Output:** `graphify-out/.graphify_labels.json`

**Note:** If LLM cannot generate good labels for all communities, label remaining as "Unknown Community N".

---

### Step 6: Report Generation

**Purpose:** Create human-readable Markdown report with all analysis findings.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path
from graphify.build import build_from_json
from graphify.report import generate
from graphify.analyze import suggest_questions

# Load all data
extract = json.loads(Path('graphify-out/.graphify_extract.json').read_text())
analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())
labels = json.loads(Path('graphify-out/.graphify_labels.json').read_text())
detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text())

# Rebuild graph
G = build_from_json(extract)

# Convert communities back to correct format
communities = {int(k): v for k, v in analysis['communities'].items()}
cohesion_scores = {int(k): v for k, v in analysis['cohesion'].items()}

# Regenerate questions with proper semantic labels
questions = suggest_questions(G, communities, community_labels=labels, top_n=7)
analysis['questions'] = questions

# Generate report - requires 10 parameters
generate(
    G,
    communities,
    labels,
    analysis['gods'],
    analysis['surprises'],
    questions,
    cohesion_scores,
    detect['total_files'],
    detect['total_words'],
    Path('graphify-out/GRAPH_REPORT.md')
)

# Update analysis file with labeled questions
Path('graphify-out/.graphify_analysis.json').write_text(json.dumps(analysis, indent=2))
print('GRAPH_REPORT.md generated')
print('Updated analysis with labeled questions')
"@
```

**Output:** `graphify-out/GRAPH_REPORT.md`

**Report Sections:**
1. Corpus Check (files, words, graph metrics)
2. Summary (nodes, edges, communities)
3. God Nodes (top 10 most connected)
4. Surprising Connections (top 10 cross-file relationships)
5. Suggested Questions (7 architectural investigation prompts)
6. Community Details (cohesion scores, node lists)
7. Isolated Nodes (documentation gaps)

---

### Step 7: HTML Visualization

**Purpose:** Generate interactive D3.js force-directed graph visualization.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path
from graphify.build import build_from_json
from graphify.export import to_html

# Load data
extract = json.loads(Path('graphify-out/.graphify_extract.json').read_text())
analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())
labels = json.loads(Path('graphify-out/.graphify_labels.json').read_text())

# Build graph
G = build_from_json(extract)

# Convert communities to correct format with int keys
communities = {int(k): v for k, v in analysis['communities'].items()}

# Generate HTML
to_html(G, Path('graphify-out/graph.html'), communities, community_labels=labels)

print(f'Graph: {G.number_of_nodes()} nodes, {G.number_of_edges()} edges')
print('graph.html generated - open in browser')
"@
```

**Output:** `graphify-out/graph.html`

**Features:**
- Force-directed layout with D3.js
- Node sizing by centrality
- Color-coded communities
- Interactive pan/zoom
- Node tooltips with labels
- Edge highlighting on hover

**Usage:** Open `graphify-out/graph.html` in any modern web browser.

---

### Step 8: Save Graph JSON

**Purpose:** Export enriched graph structure with community assignments.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())
communities = {int(k): v for k, v in analysis['communities'].items()}

Path('graphify-out/graph.json').write_text(json.dumps(communities, indent=2))
print(f'graph.json saved with {len(communities)} communities')
"@
```

**Output:** `graphify-out/graph.json`

---

### Step 9: Token Reduction Benchmark

**Purpose:** Calculate token compression efficiency.

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path
from graphify.build import build_from_json

detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text())
extract = json.loads(Path('graphify-out/.graphify_extract.json').read_text())
analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())

G = build_from_json(extract)

# Estimate tokens (1 word ≈ 1.33 tokens for English)
corpus_words = detect['total_words']
raw_tokens = int(corpus_words * 1.33)

# Graph tokens: nodes + edges + community overhead
# Each node ~15 tokens, each edge ~10 tokens, community label ~5 tokens
graph_tokens = (
    len(extract['nodes']) * 15 +
    len(extract['edges']) * 10 +
    len(analysis['communities']) * 5
)

compression = raw_tokens / graph_tokens if graph_tokens > 0 else 0

benchmark = {
    'corpus_words': corpus_words,
    'raw_tokens_estimate': raw_tokens,
    'graph_tokens_estimate': graph_tokens,
    'compression_ratio': round(compression, 1),
    'extraction_cost': {
        'input_tokens': extract.get('input_tokens', 0),
        'output_tokens': extract.get('output_tokens', 0)
    },
    'communities': len(analysis['communities']),
    'nodes': G.number_of_nodes(),
    'edges': G.number_of_edges()
}

Path('graphify-out/.graphify_benchmark.json').write_text(json.dumps(benchmark, indent=2))

print('Token Reduction Benchmark')
print('========================')
print(f'Corpus: {corpus_words:,} words')
print(f'Raw tokens (est): {raw_tokens:,}')
print(f'Graph tokens (est): {graph_tokens:,}')
print(f'Compression ratio: {compression:.2f}x')
print(f'')
print(f'Graph structure provides {compression:.1f}x more efficient context representation')
"@
```

**Output:** `graphify-out/.graphify_benchmark.json`

**Metrics:**
- Corpus word count
- Raw token estimate (words × 1.33)
- Graph token estimate (nodes + edges + communities)
- Compression ratio (raw/graph)
- Extraction cost (if using LLM extraction)

---

### Step 10: Cleanup

**Purpose:** Remove temporary files while preserving final deliverables.

```powershell
Remove-Item graphify-out/.graphify_chunk_*.json -ErrorAction SilentlyContinue
Remove-Item graphify-out/.graphify_chunk_*.txt -ErrorAction SilentlyContinue
Remove-Item graphify-out/*.py -ErrorAction SilentlyContinue
Write-Host "Cleanup complete - temporary files removed"
```

**Preserved Files:**
- `graphify-out/graph.html` - Interactive visualization
- `graphify-out/GRAPH_REPORT.md` - Analysis report
- `graphify-out/graph.json` - Graph structure
- `graphify-out/.graphify_*.json` - All intermediate results
- `graphify-out/.graphify_python` - Python interpreter cache

---

## Troubleshooting

### ImportError: cannot import name 'leiden'

**Problem:** Trying to import `leiden` function directly from `graphify.cluster`.

**Solution:** Use `cluster()` function instead. The module does not export `leiden` as a standalone function.

```python
# ❌ Wrong
from graphify.cluster import leiden

# ✅ Correct
from graphify.cluster import cluster
```

---

### TypeError: set expected at most 1 arguments, got N

**Problem:** Attempting `set(communities.values())` where values are lists.

**Solution:** `cluster()` returns `{community_id: [node_ids]}` format. Don't convert values to set.

```python
# ❌ Wrong
community_ids = set(communities.values())  # values are lists!

# ✅ Correct
community_ids = list(communities.keys())
```

---

### TypeError: god_nodes() got an unexpected keyword argument 'communities'

**Problem:** Passing `communities` parameter to `god_nodes()`.

**Solution:** In version 0.5.7+, `god_nodes()` only accepts `G` and `top_n`:

```python
# ❌ Wrong (0.4.23 API)
gods = god_nodes(G, communities=community_lists, top_n=10)

# ✅ Correct (0.5.7+ API)
gods = god_nodes(G, top_n=10)
```

---

### AttributeError: 'NoneType' object has no attribute 'get'

**Problem:** Passing `community_labels=None` to `suggest_questions()`.

**Solution:** Always provide a dict, even if temporary:

```python
# ❌ Wrong
questions = suggest_questions(G, communities, community_labels=None, top_n=7)

# ✅ Correct (minimum)
temp_labels = {cid: f"Community {cid}" for cid in communities.keys()}
questions = suggest_questions(G, communities, community_labels=temp_labels, top_n=7)

# ✅ Better (semantic labels)
labels = json.loads(Path('graphify-out/.graphify_labels.json').read_text())
questions = suggest_questions(G, communities, community_labels=labels, top_n=7)
```

---

### Terminal Output Truncates to "hon"

**Problem:** PowerShell terminal consistently shows truncated output ending with "hon".

**Solution:** This is a display issue only. Scripts execute successfully. Verify completion by checking file existence:

```powershell
# Don't rely on terminal output
& $PYTHON script.py

# Instead, verify file was created
if (Test-Path "graphify-out/.graphify_analysis.json") {
    Write-Host "✓ Analysis complete"
    Get-Content "graphify-out/.graphify_analysis.json" | ConvertFrom-Json | Format-List
}
```

---

### Community Labels Show "Unknown Community N"

**Problem:** LLM failed to generate semantic labels for some communities.

**Solution:** Manually inspect those communities and generate labels:

```powershell
$PYTHON = Get-Content "graphify-out/.graphify_python"
& $PYTHON -c @"
import json
from pathlib import Path

analysis = json.loads(Path('graphify-out/.graphify_analysis.json').read_text())
communities = {int(k): v for k, v in analysis['communities'].items()}

# Find unlabeled communities
labels = json.loads(Path('graphify-out/.graphify_labels.json').read_text())
for cid, nodes in communities.items():
    label = labels.get(str(cid), '')
    if label.startswith('Unknown'):
        print(f'Community {cid}: {nodes[:5]}')  # Show first 5 nodes
"@
```

Then use an LLM to generate labels for those specific communities and update the labels file.

---

## Output File Reference

| File | Purpose | When Generated |
|------|---------|----------------|
| `.graphify_python` | Python interpreter path cache | Step 0 |
| `.graphify_detect.json` | File categorization results | Step 2 |
| `.graphify_ast.json` | AST extraction from code | Step 3A |
| `.graphify_semantic.json` | Semantic extraction from docs | Step 3B |
| `.graphify_extract.json` | Merged AST + semantic | Step 3C |
| `.graphify_analysis.json` | Graph analysis results | Step 4 |
| `.graphify_prep_labels.json` | Sample nodes for labeling | Step 5 |
| `.graphify_labels.json` | Community semantic labels | Step 5 |
| `GRAPH_REPORT.md` | Human-readable analysis report | Step 6 |
| `graph.html` | Interactive D3.js visualization | Step 7 |
| `graph.json` | Graph structure with communities | Step 8 |
| `.graphify_benchmark.json` | Token compression metrics | Step 9 |

### Intermediate Files (Deleted in Step 10)

| File Pattern | Purpose |
|-------------|---------|
| `.graphify_chunk_*.txt` | Document file lists for parallel extraction |
| `.graphify_chunk_*.json` | Parallel semantic extraction results |
| `*.py` | Temporary Python scripts |

---

## Performance Considerations

### Parallel Semantic Extraction

For large document sets (100+ files), always use parallel extraction:

- **6 parallel Explore subagents** = optimal for most repositories
- Each subagent processes ~15-25 files independently
- Total extraction time ≈ longest single chunk time (not sum of all chunks)

### Memory Usage

Large graphs (1000+ nodes) may require increased Python memory:

```powershell
$env:PYTHONMALLOC = "malloc"
$env:MALLOC_MMAP_THRESHOLD_ = "4096"
```

### LLM Token Costs

Semantic extraction is the primary LLM cost:
- **Detection:** ~500 tokens
- **AST:** 0 tokens (static analysis)
- **Semantic per file:** ~200-500 tokens input + ~150-300 tokens output
- **Labeling:** ~100 tokens per community
- **Questions:** ~500 tokens

**Total estimate:** ~(document_count × 400) + (community_count × 100) tokens

---

## Best Practices

### 1. Always Cache Python Interpreter

```powershell
# First command in any graphify session
$PYTHON = (Get-Command python).Source
$PYTHON | Out-File -FilePath "graphify-out/.graphify_python" -Encoding utf8 -NoNewline
```

This ensures all pipeline steps use the same Python environment.

### 2. Verify File Existence, Not Terminal Output

```powershell
# ❌ Don't rely on terminal output
& $PYTHON script.py
# (output might be truncated)

# ✅ Verify files were created
if (Test-Path "graphify-out/.graphify_analysis.json") {
    Write-Host "✓ Step complete"
}
```

### 3. Always Provide community_labels

```python
# ❌ Never do this
questions = suggest_questions(G, communities, community_labels=None, top_n=7)

# ✅ Always provide dict
temp_labels = {cid: f"Community {cid}" for cid in communities.keys()}
questions = suggest_questions(G, communities, community_labels=temp_labels, top_n=7)
```

### 4. Use Semantic Labels for Final Report

Generate temporary labels first (Step 4), then regenerate with semantic labels (Step 6):

```python
# Step 4: Quick analysis with temp labels
temp_labels = {cid: f"Community {cid}" for cid in communities.keys()}
questions = suggest_questions(G, communities, community_labels=temp_labels, top_n=7)

# Step 6: Regenerate with semantic labels for report
labels = json.loads(Path('graphify-out/.graphify_labels.json').read_text())
questions = suggest_questions(G, communities, community_labels=labels, top_n=7)
```

### 5. Preserve Intermediate Files for Debugging

Don't delete intermediate JSON files until pipeline fully completes. They're essential for troubleshooting API issues.

---

## Quick Reference: API Signatures (v0.5.7+)

```python
from graphify.detect import detect_corpus
from graphify.extract import collect_files, extract
from graphify.build import build_from_json
from graphify.cluster import cluster
from graphify.analyze import god_nodes, surprising_connections, suggest_questions, cohesion
from graphify.report import generate
from graphify.export import to_html

# Detection
result = detect_corpus(Path('.'), output_dir=Path('graphify-out'))

# Extraction
nodes_edges = extract(code_files)  # AST

# Build
G = build_from_json(extract_data)

# Analysis
communities = cluster(G)  # Returns {community_id: [node_ids]}
gods = god_nodes(G, top_n=10)  # NO communities param
surprises = surprising_connections(G, communities=communities, top_n=5)
questions = suggest_questions(G, communities, community_labels=labels_dict, top_n=7)  # REQUIRES labels dict
score = cohesion(G, node_list)

# Report
generate(G, communities, labels, gods, surprises, questions, cohesion_scores, 
         total_files, total_words, output_path)

# Export
to_html(G, output_path, communities, community_labels=labels)
```

---

## Example: Complete Pipeline Script

```powershell
# Save as: Run-GraphifyPipeline.ps1
param(
    [string]$RepoPath = ".",
    [string]$OutputDir = "graphify-out"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "Golem-Agents-Legion: graphify Pipeline" -ForegroundColor Cyan
Write-Host "Repository: $RepoPath"
Write-Host "Output: $OutputDir"
Write-Host ""

# Step 0: Cache Python
Write-Host "Step 0: Caching Python interpreter..." -ForegroundColor Yellow
$PYTHON = (Get-Command python).Source
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$PYTHON | Out-File -FilePath "$OutputDir/.graphify_python" -Encoding utf8 -NoNewline
Write-Host "✓ Python: $PYTHON" -ForegroundColor Green

# Step 1: Verify Installation
Write-Host "`nStep 1: Verifying graphify installation..." -ForegroundColor Yellow
& $PYTHON -c "import graphify; print(f'✓ graphify {graphify.__version__}')"

# Step 2: File Detection
Write-Host "`nStep 2: Detecting files..." -ForegroundColor Yellow
& $PYTHON -c @"
from graphify.detect import detect_corpus
from pathlib import Path
result = detect_corpus(Path('$RepoPath'), output_dir=Path('$OutputDir'))
print(f'✓ Files: {result["total_files"]}, Words: {result["total_words"]:,}')
"@

# Step 3A: AST Extraction
Write-Host "`nStep 3A: Extracting AST from code..." -ForegroundColor Yellow
& $PYTHON -c @"
import json
from graphify.extract import collect_files, extract
from pathlib import Path
detect = json.loads(Path('$OutputDir/.graphify_detect.json').read_text())
code_files = []
for f in detect.get('files', {}).get('code', []):
    code_files.extend(collect_files(Path(f)) if Path(f).is_dir() else [Path(f)])
if code_files:
    result = extract(code_files)
    Path('$OutputDir/.graphify_ast.json').write_text(json.dumps(result, indent=2))
    print(f'✓ AST: {len(result["nodes"])} nodes, {len(result["edges"])} edges')
else:
    Path('$OutputDir/.graphify_ast.json').write_text(json.dumps({'nodes':[],'edges':[],'input_tokens':0,'output_tokens':0}))
    print('✓ No code files - skipping AST')
"@

# Step 3B: Semantic Extraction
Write-Host "`nStep 3B: Semantic extraction..." -ForegroundColor Yellow
Write-Host "NOTE: This step requires manual parallel Explore subagent execution." -ForegroundColor Cyan
Write-Host "      See full documentation for chunk preparation and merging." -ForegroundColor Cyan
Write-Host "      Assuming .graphify_semantic.json already exists..." -ForegroundColor Cyan

# Step 3C: Merge
Write-Host "`nStep 3C: Merging AST + Semantic..." -ForegroundColor Yellow
& $PYTHON -c @"
import json
from pathlib import Path
ast = json.loads(Path('$OutputDir/.graphify_ast.json').read_text())
semantic = json.loads(Path('$OutputDir/.graphify_semantic.json').read_text())
merged = {'nodes': ast.get('nodes', []) + semantic.get('nodes', []), 'edges': ast.get('edges', []) + semantic.get('edges', []), 'hyperedges': semantic.get('hyperedges', []), 'input_tokens': 0, 'output_tokens': 0}
Path('$OutputDir/.graphify_extract.json').write_text(json.dumps(merged, indent=2))
print(f'✓ Total: {len(merged["nodes"])} nodes, {len(merged["edges"])} edges')
"@

# Step 4: Analysis
Write-Host "`nStep 4: Analyzing graph..." -ForegroundColor Yellow
& $PYTHON -c @"
import json
from pathlib import Path
from graphify.build import build_from_json
from graphify.cluster import cluster
from graphify.analyze import god_nodes, surprising_connections, suggest_questions, cohesion
extract = json.loads(Path('$OutputDir/.graphify_extract.json').read_text())
G = build_from_json(extract)
communities = cluster(G)
cohesion_scores = {cid: cohesion(G, nodes) for cid, nodes in communities.items()}
gods = god_nodes(G, top_n=10)
surprises = surprising_connections(G, communities=communities, top_n=10)
temp_labels = {cid: f'Community {cid}' for cid in communities.keys()}
questions = suggest_questions(G, communities, community_labels=temp_labels, top_n=7)
analysis = {'communities': {str(k): v for k, v in communities.items()}, 'cohesion': {str(k): v for k, v in cohesion_scores.items()}, 'gods': gods, 'surprises': surprises, 'questions': questions}
Path('$OutputDir/.graphify_analysis.json').write_text(json.dumps(analysis, indent=2))
print(f'✓ {len(communities)} communities, {len(gods)} god nodes, {len(surprises)} surprises')
"@

Write-Host "`nPipeline Steps 5-10 require additional manual steps." -ForegroundColor Cyan
Write-Host "See full documentation for:" -ForegroundColor Cyan
Write-Host "  - Step 5: Community labeling (requires LLM)" -ForegroundColor Cyan
Write-Host "  - Step 6: Report generation" -ForegroundColor Cyan
Write-Host "  - Step 7: HTML visualization" -ForegroundColor Cyan
Write-Host "  - Step 8: Graph JSON export" -ForegroundColor Cyan
Write-Host "  - Step 9: Token benchmark" -ForegroundColor Cyan
Write-Host "  - Step 10: Cleanup" -ForegroundColor Cyan

Write-Host "`n✓ Pipeline complete through Step 4" -ForegroundColor Green
```

---

## Related Documentation

- graphify SKILL.md (note: written for v0.4.23 - API signatures differ)
- graphify GitHub: https://github.com/dylanhogg/graphify
- NetworkX Documentation: https://networkx.org/
- D3.js Force-Directed Graphs: https://d3js.org/

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-05-01 | Initial documentation based on Golem-Agents-Legion pipeline execution with graphify 0.5.7 |

---

**Target Audience:** LLMs (including local models like google/gemma-4-E4B), AI agents, and human developers who need to execute the complete graphify knowledge graph extraction pipeline with API version 0.5.7+.

**Prerequisites:** Python environment with graphify installed, PowerShell on Windows (or bash equivalent on Unix), and LLM access for semantic extraction and community labeling.
