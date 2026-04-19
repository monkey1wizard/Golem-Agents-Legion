# Graph Report - C:\Code\Golem-Agents-Legion  (2026-04-19)

## Corpus Check
- 33 files · ~151,673 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 210 nodes · 274 edges · 28 communities detected
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 15 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]

## God Nodes (most connected - your core abstractions)
1. `MCPConnection` - 11 edges
2. `run_loop()` - 9 edges
3. `create_connection()` - 7 edges
4. `ReviewHandler` - 7 edges
5. `MCPConnectionStdio` - 6 edges
6. `MCPConnectionSSE` - 6 edges
7. `MCPConnectionHTTP` - 6 edges
8. `run_evaluation()` - 6 edges
9. `find_runs()` - 6 edges
10. `parse_skill_md()` - 6 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `create_connection()`  [INFERRED]
  C:\Code\Golem-Agents-Legion\skills\mcp-builder\scripts\evaluation.py → C:\Code\Golem-Agents-Legion\skills\mcp-builder\scripts\connections.py
- `is_server_ready()` --calls--> `create_connection()`  [INFERRED]
  C:\Code\Golem-Agents-Legion\skills\webapp-testing\scripts\with_server.py → C:\Code\Golem-Agents-Legion\skills\mcp-builder\scripts\connections.py
- `fill_pdf_fields()` --calls--> `get_field_info()`  [INFERRED]
  C:\Code\Golem-Agents-Legion\skills\pdf\scripts\fill_fillable_fields.py → C:\Code\Golem-Agents-Legion\skills\pdf\scripts\extract_form_field_info.py
- `run_loop()` --calls--> `generate_html()`  [INFERRED]
  C:\Code\Golem-Agents-Legion\skills\skill-creator\scripts\run_loop.py → C:\Code\Golem-Agents-Legion\skills\skill-creator\scripts\generate_report.py
- `main()` --calls--> `generate_html()`  [INFERRED]
  C:\Code\Golem-Agents-Legion\skills\skill-creator\scripts\run_loop.py → C:\Code\Golem-Agents-Legion\skills\skill-creator\scripts\generate_report.py

## Communities

### Community 0 - "Community 0"
Cohesion: 0.11
Nodes (24): Convert-ToTomlMultilineLiteralString(), ConvertTo-CodexMcpSection(), ConvertTo-OrderedMap(), ConvertTo-TomlArray(), ConvertTo-TomlString(), Ensure-Ripgrep(), Get-ConfiguredValue(), Get-McpVariableMap() (+16 more)

### Community 1 - "Community 1"
Cohesion: 0.1
Nodes (23): generate_html(), main(), Generate HTML report from loop output data. If auto_refresh is True, adds a meta, _call_claude(), improve_description(), main(), Run `claude -p` with the prompt on stdin and return the text response.      Prom, Call Claude to improve the description based on eval results. (+15 more)

### Community 2 - "Community 2"
Cohesion: 0.12
Nodes (14): ABC, create_connection(), MCPConnection, MCPConnectionHTTP, MCPConnectionSSE, MCPConnectionStdio, Lightweight connection handling for MCP servers., MCP connection using Streamable HTTP. (+6 more)

### Community 3 - "Community 3"
Cohesion: 0.13
Nodes (18): BaseHTTPRequestHandler, build_run(), embed_file(), find_runs(), _find_runs_recursive(), generate_html(), get_mime_type(), _kill_port() (+10 more)

### Community 4 - "Community 4"
Cohesion: 0.13
Nodes (18): Retrieve available tools from the MCP server., Call a tool on the MCP server with provided arguments., agent_loop(), evaluate_single_task(), extract_xml_content(), main(), parse_env_vars(), parse_evaluation_file() (+10 more)

### Community 5 - "Community 5"
Cohesion: 0.27
Nodes (6): Get-ActivePlanPath(), Get-RepoContextRoot(), Get-StateContext(), Get-StatePath(), Resolve-PlanPath(), Unwrap-MarkdownCode()

### Community 6 - "Community 6"
Cohesion: 0.24
Nodes (11): aggregate_results(), calculate_stats(), generate_benchmark(), generate_markdown(), load_run_results(), main(), Aggregate run results into summary statistics.      Returns run_summary with sta, Generate complete benchmark.json from run results. (+3 more)

### Community 7 - "Community 7"
Cohesion: 0.33
Nodes (6): get_field_info(), get_full_annotation_field_id(), make_field_dict(), write_field_info(), fill_pdf_fields(), validation_error_for_field_value()

### Community 8 - "Community 8"
Cohesion: 0.28
Nodes (7): main(), package_skill(), Check if a path should be excluded from packaging., Package a skill folder into a .skill file.      Args:         skill_path: Path t, should_exclude(), Basic validation of a skill, validate_skill()

### Community 9 - "Community 9"
Cohesion: 0.43
Nodes (5): Add-SourceBlock(), Build-AdapterContent(), Get-AllSkillSources(), Get-ConventionSources(), Read-NormalizedFile()

### Community 10 - "Community 10"
Cohesion: 0.67
Nodes (3): is_server_ready(), main(), Wait for server to be ready by polling the port.

### Community 11 - "Community 11"
Cohesion: 0.67
Nodes (3): extract_form_structure(), main(), Extract form structure from a non-fillable PDF.  This script analyzes the PDF

### Community 12 - "Community 12"
Cohesion: 0.83
Nodes (3): fill_pdf_form(), transform_from_image_coords(), transform_from_pdf_coords()

### Community 13 - "Community 13"
Cohesion: 1.0
Nodes (2): get_bounding_box_messages(), RectAndField

### Community 14 - "Community 14"
Cohesion: 1.0
Nodes (0): 

### Community 15 - "Community 15"
Cohesion: 1.0
Nodes (0): 

### Community 16 - "Community 16"
Cohesion: 1.0
Nodes (0): 

### Community 17 - "Community 17"
Cohesion: 1.0
Nodes (0): 

### Community 18 - "Community 18"
Cohesion: 1.0
Nodes (0): 

### Community 19 - "Community 19"
Cohesion: 1.0
Nodes (0): 

### Community 20 - "Community 20"
Cohesion: 1.0
Nodes (0): 

### Community 21 - "Community 21"
Cohesion: 1.0
Nodes (0): 

### Community 22 - "Community 22"
Cohesion: 1.0
Nodes (0): 

### Community 23 - "Community 23"
Cohesion: 1.0
Nodes (1): Create the connection context based on connection type.

### Community 24 - "Community 24"
Cohesion: 1.0
Nodes (0): 

### Community 25 - "Community 25"
Cohesion: 1.0
Nodes (0): 

### Community 26 - "Community 26"
Cohesion: 1.0
Nodes (0): 

### Community 27 - "Community 27"
Cohesion: 1.0
Nodes (0): 

## Knowledge Gaps
- **46 isolated node(s):** `Lightweight connection handling for MCP servers.`, `Base class for MCP server connections.`, `Create the connection context based on connection type.`, `Initialize MCP server connection.`, `Clean up MCP server connection resources.` (+41 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 14`** (2 nodes): `Start-GalWorker.ps1`, `Write-StatusJson()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 15`** (2 nodes): `convert_pdf_to_images.py`, `convert()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 16`** (2 nodes): `create_validation_image.py`, `create_validation_image()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 17`** (2 nodes): `read_pdf.py`, `extract_pdf_to_text()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 18`** (2 nodes): `console_logging.py`, `handle_console_message()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 19`** (1 nodes): `Get-GalRemoteResult.ps1`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 20`** (1 nodes): `Init-Repo.ps1`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 21`** (1 nodes): `Invoke-GalRemoteTask.ps1`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 22`** (1 nodes): `Uninstall-Machine.ps1`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 23`** (1 nodes): `Create the connection context based on connection type.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 24`** (1 nodes): `check_fillable_fields.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 25`** (1 nodes): `__init__.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 26`** (1 nodes): `element_discovery.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 27`** (1 nodes): `static_html_automation.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `MCPConnection` connect `Community 2` to `Community 4`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **Why does `create_connection()` connect `Community 2` to `Community 10`, `Community 4`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **Why does `main()` connect `Community 4` to `Community 2`?**
  _High betweenness centrality (0.014) - this node is a cross-community bridge._
- **Are the 5 inferred relationships involving `run_loop()` (e.g. with `find_project_root()` and `parse_skill_md()`) actually correct?**
  _`run_loop()` has 5 INFERRED edges - model-reasoned connections that need verification._
- **Are the 2 inferred relationships involving `create_connection()` (e.g. with `main()` and `is_server_ready()`) actually correct?**
  _`create_connection()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Lightweight connection handling for MCP servers.`, `Base class for MCP server connections.`, `Create the connection context based on connection type.` to the rest of the system?**
  _46 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.11 - nodes in this community are weakly interconnected._