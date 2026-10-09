# GAL writing checks

This directory contains the repository-owned, pinned textlint toolchain. It is a development tool. Installed GAL copies and downstream repositories do not need this Node workspace.

## Install and run

From the repository root, install the lockfile exactly once:

```text
npm ci --prefix tools/writing
```

Run the required tracked corpus:

```text
node tools/writing/check.mjs --required
```

Run selected files with `--files`. Relative input paths resolve from the current working directory. The checker uses that directory as the document and terminology-authority root. Its pinned configuration, rules, and textlint CLI always resolve from this tool's location. The checker selects each file's profile in this order: explicit `--locale`, top-level frontmatter `lang`, then a recognized managed path. Without an explicit profile, every file must resolve independently.

```text
node tools/writing/check.mjs --files README.md
node tools/writing/check.mjs --files docs/i18n/zh-Hant/terminology.zh-Hant.md
node tools/writing/check.mjs --files docs/i18n/ja/terminology.ja.md
node tools/writing/check.mjs --locale en-US --files draft.md
```

Use `--transport mcp` to run the same complete GAL check through the official textlint MCP server. The default transport is `cli`. Put options before `--files`, which consumes the remaining file paths. Both transports use the same locale selection, terminology validation, diagnostic normalization, and report format.

```text
node tools/writing/check.mjs --transport mcp --locale en-US --files draft.md
node tools/writing/check.mjs --transport mcp --required
```

The MCP transport launches the pinned local `textlint --mcp` server through its installed SDK. It checks the captured input bytes with `lintText`. For an empty file, it uses the official `lintFile` tool because `lintText` rejects empty strings. Both transports reject a result if an input file has changed before report completion. This check detects a remaining change, not every transient edit that another process might restore during the run.

Verify that the terminology snapshot for the current document root is current:

```text
node tools/writing/terms.mjs --check
```

The raw official textlint MCP entry point is also available after installation. Raw CLI/MCP commands are low-level diagnostic primitives and return upstream data. They do not provide the complete GAL finding metadata or execution report. Use `check.mjs` with the chosen transport when complete GAL evidence is required. To inspect the raw MCP interface, resolve both executable paths to trusted files in this checkout. Start it from the document root so relative stdin identity and terminology authorities use that root:

```text
node tools/writing/node_modules/textlint/bin/textlint.js --mcp --config "$PWD/tools/writing/config/en-US.json" --rules-base-directory "$PWD/tools/writing/node_modules"
```

On PowerShell, use the equivalent absolute paths:

```powershell
$writingRoot = (Resolve-Path tools/writing).Path
node "$writingRoot/node_modules/textlint/bin/textlint.js" --mcp --config "$writingRoot/config/en-US.json" --rules-base-directory "$writingRoot/node_modules"
```

The MCP caller must choose a trusted repository-owned JSON profile. The terminology rule reads authority files and a matching immutable snapshot named by their SHA-256 source hash from the current document root. The rule fails with an operational error when that snapshot is missing or invalid. The checker publishes a complete snapshot before it starts textlint, then pins the source hash in its child process. The child verifies the pin against the authority files it reads. Direct CLI and MCP calls resolve the current root without extra settings and require a snapshot already published by the checker or `terms.mjs`. The checker does not discover arbitrary repository JavaScript configuration, download packages, repair text, or modify input files. For stdin, pass a virtual filename through the textlint interface. The parent checker uses file paths and does not persist chat text by default.

The native CLI and MCP routes require trusted, repository-owned configuration. Resolve the configuration and rules paths from the repository root. Do not pass a repository JavaScript configuration file. For stdin, provide the virtual filename explicitly:

```text
node tools/writing/node_modules/textlint/bin/textlint.js --config "$PWD/tools/writing/config/en-US.json" --rules-base-directory "$PWD/tools/writing/node_modules" --stdin --stdin-filename draft.md
```

The native CLI and MCP commands above are examples for a POSIX shell. In PowerShell, resolve `tools/writing` as shown in the MCP example and pass the selected profile and virtual filename explicitly.

## Results and guarantees

The three profiles are `en-US`, `zh-TW`, and `ja-JP`. Each profile uses the fixed roster and the native protection filter. The local terminology rule owns project terminology. Each terminology profile has separate hard/error and advisory/warning channels. The English and Japanese heuristic rules are advisory. Native CLI and MCP severity therefore match checker severity. See [rule-provenance.md](rule-provenance.md) for rule ownership, versions, licenses, adapted authorities, and policy boundaries. See the three terminology authorities listed by `terms.mjs` for executable term data.

The checker uses a 60-second deadline for each locale's textlint child process, including MCP initialization and checks. It terminates only its own child and requires confirmation that it closed. MCP cleanup uses the SDK's bounded shutdown grace period after the deadline. It prints at most 50 findings. It writes the complete atomic JSON report below `.dev/pipeline/writing-quality/<request-id>/report.json`, under the current document root. Reports include schema version, request ID, completion status, transport, selected and skipped files with reasons, per-file findings, source and config hashes, installed tool versions, selected profiles, terminology source hash, policy sources, and hard findings. The `servers` array records each MCP server's advertised identity after a successful initialization handshake, including when a later check fails. It remains empty if no handshake completed. Infrastructure failures also write a failure report with completed locale results preserved. The report directory is runtime output and is ignored.

Exit codes are:

- `0`: the check completed with no hard findings.
- `1`: the check completed with one or more hard findings.
- `2`: an infrastructure failure occurred, such as missing configuration, stale terminology data, missing selected files, incomplete or malformed tool output, or a timeout.

The checker accepts `--required` or `--files <paths...>` with an optional `--locale <locale>` override. A supplied `--locale` must have a supported value. An empty or missing value fails with exit code 2 and does not fall back to metadata or a path. Required mode filters `git ls-files -z` to exactly `README.md`, `CONTRIBUTING.md`, `.dev/project.md`, Markdown files under `docs/`, and Markdown files under `plugins/gal-core/`. It skips no tracked required paths silently. Supported frontmatter `lang` values are `en`, `en-US`, `ja`, `ja-JP`, and `zh-TW`. The value `zh-Hant` is supported only when the input resolves under the document root's managed `docs/i18n/zh-Hant/` path. Explicit `--locale` accepts `en`, `en-US`, `ja`, `ja-JP`, and `zh-TW`; it normalizes `en` to `en-US` and `ja` to `ja-JP`. The bounded frontmatter parser accepts an optional BOM, standalone `---` delimiters, and one scalar `lang` line with an unquoted, single-quoted, or double-quoted value. It is not general YAML. The checker uses the resolved, root-relative identity for managed-path selection and `zh-Hant` metadata, so absolute paths, `./` segments, and internal `..` segments select the same file and locale. Duplicate, empty, compound, malformed, or unsupported metadata fails with exit code 2 unless a valid explicit locale overrides it. A valid override does not inspect unused metadata. Known English paths are `README.md`, `CONTRIBUTING.md`, `.dev/project.md`, Markdown below `docs/` excluding `docs/i18n/`, and Markdown below `plugins/gal-core/`. Known translation paths are `docs/i18n/zh-Hant/` and `docs/i18n/ja/`; other paths need metadata or an explicit override. Virtual filenames do not select language. Mixed language content does not enable multiple profiles. Reports retain the caller's input path and record the selected locale and whether it came from an explicit override, frontmatter, or a managed path.

The report uses `schemaVersion=1`. Each selected-file record contains its input path, selected locale, selection reason, SHA-256 input hash, and findings. Each finding contains the textlint rule ID, severity, source range, rationale, suggestion, and policy source. The top level contains selected and skipped records, full findings, hard findings, package versions, config hashes, profile names, terms provenance, completion status, and any infrastructure error. A clean result is evidence for review. It does not certify semantic equivalence or readability. No automatic repair is performed, and input bytes are not rewritten. See [rule-provenance.md](rule-provenance.md) for rule ownership, exact versions, licenses, and policy sources.

Each normalized finding has `suggestion: {text, replacement, applicability: "manual"}`. The text describes an action for review. The replacement is an authority-defined candidate string or `null` when no specific replacement is justified. Terminology findings suggest the preferred or required first-use form. Context-only findings ask the reader to review the occurrence. English and Japanese heuristic suggestions explain the applicable policy without inventing an actor, a fact, or a replacement sentence. A manual suggestion is not a safe automatic fix. Any upstream `fix` field is preserved as evidence and is not applied by the checker.

Native CLI and official MCP terminology messages carry these fields under `data.suggestion` and `data.sourceAnchor`. The checker also exposes `suggestion`, `sourceAnchor`, and the matching `policySource` at the finding's top level. This metadata transport depends on the reported-object support in pinned textlint 15.2.3. `RuleError` discards custom fields in that version. The rule explicitly retains the configured severity when it reports an object. Integration tests cover the native CLI, official MCP, source ranges, warning severity, and protected input. Third-party native messages retain their upstream format. The checker adds policy suggestions when it normalizes those messages.

In textlint 15.2.3, the official MCP server's operational-error object does not satisfy its own `lintText` output schema. An SDK client that validates tool output therefore receives MCP error `-32602`. The underlying protocol response has `isError: true` and the operational cause. Both results indicate a failed check. They must not be treated as a clean result.

## Terminology data

The generated terminology data uses `schemaVersion=1`, with a SHA-256 `sourceHash` and an `entries` array. Each entry contains the following fields. The report has its own `schemaVersion=1`; report records and terminology entries have different structures.

| Field | Meaning |
| --- | --- |
| `conceptId` | Stable ID connecting a locale entry to an English canonical concept. |
| `locale` | Explicit `en-US`, `zh-TW`, or `ja-JP` profile. |
| `preferred` | Preferred written form. |
| `allowed` | Permitted variants. |
| `forbidden` | Rejected variants. |
| `mode` | `keep-en`, `localized`, `bilingual-first-use`, `transliterated`, or `contextual` presentation. |
| `scope` | Applicable prose scopes. |
| `exceptions` | Declared exceptions retained from the authority data. |
| `sourceAnchor` | Reference to the authority path and anchor. |
| `sourceRow` | Optional identification of the human table row validated against the structured entry. |

`terms.mjs` builds these entries from explicit blocks in the three terminology authorities. The source fields `allowed`, `forbidden`, `scope`, and `exceptions` must each be explicit arrays of strings. Missing fields, nulls, and scalar shorthand fail before normalization. It validates IDs, required fields, source anchors, presentation modes, list types, human-table consistency, and conflicting allowed/forbidden forms. It publishes complete, immutable snapshots at `.dev/cache/writing-terms/<sourceHash>.json`. A valid existing snapshot is reused without replacement. Rules are not inferred from ordinary prose. See [rule-provenance.md](rule-provenance.md#runtime-and-parser) for exact dependency versions and licenses.

## Locale and terminology conflicts

Managed `docs/i18n/zh-Hant/` paths use the `zh-TW` profile. That mapping does not claim that all Traditional Chinese is Taiwan Traditional Chinese. Project terminology authorities win conflicts with supplemental regional or contextual advice. An optional `zhtw-mcp` result is supplemental and must not replace the local terminology check or introduce a second punctuation policy.

## Tests

Run the focused checker tests and the existing writing-tool tests from the repository root:

```text
node --test tools/writing/tests/check.test.mjs
node --test tools/writing/tests/prose.test.mjs
node --test tools/writing/tests/terms.test.mjs tools/writing/tests/terminology.test.mjs
```

The `npm ci` command installs only the lockfile-pinned local toolchain. Verify its dependency versions with `npm ls --prefix tools/writing --depth=0`; do not run installation during ordinary checks.
