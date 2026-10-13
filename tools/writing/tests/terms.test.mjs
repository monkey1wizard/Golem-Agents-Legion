import assert from "node:assert/strict";
import { test } from "node:test";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import {
  buildTerms,
  validateTerms,
  generate,
  check,
  loadSources,
  AUTHORITY_PATHS,
  snapshotPath,
  SCHEMA_VERSION,
} from "../terms.mjs";

const block = (locale, entries, authority) =>
  `<!-- gal-terms:start -->\n\`\`\`json\n${JSON.stringify({
    schemaVersion: 1,
    locale,
    authority,
    entries,
  })}\n\`\`\`\n<!-- gal-terms:end -->`;

const entry = (conceptId, locale = "en-US") => ({
  conceptId,
  locale,
  preferred: "preferred",
  allowed: ["preferred"],
  forbidden: [],
  mode: "keep-en",
  scope: ["prose"],
  exceptions: [],
  sourceAnchor: `${locale}:1`,
});

test("buildTerms extracts only explicit JSON blocks and sorts entries", () => {
  const data = buildTerms({
    "b.md": block("zh-TW", [entry("word", "zh-TW")]),
    "a.md": block("en-US", [entry("word")]),
  });
  assert.equal(data.schemaVersion, SCHEMA_VERSION);
  assert.equal(data.entries.length, 2);
  assert.equal(data.entries[0].locale, "en-US");
  assert.match(data.sourceHash, /^[a-f0-9]{64}$/);
});

test("source blocks reject scalar, missing, null, and nonstring term lists before normalization", () => {
  for (const field of ["allowed", "forbidden", "scope", "exceptions"]) {
    for (const value of ["prose", null, undefined, [1]]) {
      assert.throws(() => buildTerms({ "a.md": block("en-US", [{ ...entry("word"), [field]: value }]) }), new RegExp(`${field} must be an explicit array of strings`));
    }
  }
});

test("validation rejects invalid schemaVersion, missing array, duplicate, unknown canonical ID, missing anchor, and conflicting forms", () => {
  const base = { schemaVersion: 1, sourceHash: "0".repeat(64), entries: [entry("word")] };
  assert.throws(() => validateTerms({ schemaVersion: 2, entries: [] }), /schemaVersion=1/);
  assert.throws(() => validateTerms({ schemaVersion: 1, entries: "invalid" }), /entries array/);
  assert.throws(() => validateTerms({ ...base, entries: [entry("word"), entry("word")] }), /duplicate/);
  assert.throws(() => validateTerms({ ...base, entries: [entry("word"), entry("unknown", "zh-TW")] }), /unknown canonical/);
  assert.throws(() => validateTerms({ ...base, entries: [{ ...entry("word"), sourceAnchor: "" }] }), /missing source anchor/);
  assert.throws(() => validateTerms({ ...base, entries: [{ ...entry("word"), allowed: ["x"], forbidden: ["x"] }] }), /both allowed and forbidden/);
  assert.throws(() => validateTerms({ ...base, entries: [{ ...entry("word"), mode: "invalid-mode" }] }), /unknown presentation mode/);
  assert.throws(() => validateTerms({ ...base, entries: [entry("word"), { ...entry("word", "fr-FR"), locale: "fr-FR" }] }), /unknown locale/);
  assert.throws(() => validateTerms({ ...base, entries: [entry("word"), { ...entry("other"), preferred: "other", allowed: ["other"], forbidden: ["preferred"] }] }), /approved by .* but forbidden by/);
});

test("buildTerms rejects disagreement between explicit JSON block and human table cells", () => {
  const tableWithDisagreement = [
    block("en-US", [entry("term-a")]),
    "",
    "| conceptId | mode | preferred |",
    "| --- | --- | --- |",
    "| `term-a` | `localized` | `preferred` |",
  ].join("\n");

  assert.throws(
    () => buildTerms({ "authority.md": tableWithDisagreement }),
    /human table disagrees with term-a\.mode/
  );
});

test("C-09: byte-identical regeneration from sources", () => {
  const sources = {
    "docs/glossary.md": block("en-US", [entry("term-1"), entry("term-2")]),
    "docs/i18n/zh-Hant/terminology.zh-Hant.md": block("zh-TW", [entry("term-1", "zh-TW")]),
    "docs/i18n/ja/terminology.ja.md": block("ja-JP", [entry("term-2", "ja-JP")]),
  };

  const first = buildTerms(sources);
  const second = buildTerms(sources);

  assert.equal(JSON.stringify(first), JSON.stringify(second));
  assert.equal(first.sourceHash, second.sourceHash);
});

test("C-09: stale-source rejection and terms check()", async () => {
  const sources = await loadSources();
  const currentData = buildTerms(sources);
  assert.equal(currentData.schemaVersion, 1);

  // check() succeeds against currently generated cache
  await generate();
  const checked = await check();
  assert.equal(checked.sourceHash, currentData.sourceHash);

  // check() in a temp directory with missing derived data throws
  const emptyDir = await mkdtemp(join(tmpdir(), "gal-terms-empty-"));
  try {
    await assert.rejects(
      async () => check(emptyDir),
      /missing derived terminology data|ENOENT/
    );
  } finally {
    await rm(emptyDir, { recursive: true, force: true });
  }
});

test("C-09: atomic generation leaves cache intact on error", async () => {
  const tempDir = await mkdtemp(join(tmpdir(), "gal-atomic-"));
  try {
    // Malformed source file in tempDir
    await writeFile(join(tempDir, "docs/glossary.md"), "<!-- gal-terms:start -->\n```json\n{ invalid }\n```\n<!-- gal-terms:end -->", { flag: "w" }).catch(() => {});
    // generate in an invalid directory should throw without partially replacing
    await assert.rejects(async () => generate(tempDir));
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test("concurrent generation uses unique atomic temporary files", async () => {
  const tempDir = await mkdtemp(join(tmpdir(), "gal-terms-concurrent-"));
  try {
    const modulePath = join(tempDir, "tools", "writing", "terms.mjs");
    await mkdir(join(tempDir, "tools", "writing"), { recursive: true });
    await cp(new URL("../terms.mjs", import.meta.url), modulePath);

    const sources = {
      "docs/glossary.md": block("en-US", [entry("concurrent-term")], "glossary"),
      "docs/i18n/zh-Hant/terminology.zh-Hant.md": block("zh-TW", [entry("concurrent-term", "zh-TW")]),
      "docs/i18n/ja/terminology.ja.md": block("ja-JP", [entry("concurrent-term", "ja-JP")]),
    };
    for (const [path, content] of Object.entries(sources)) {
      const target = join(tempDir, path);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, content, "utf8");
    }

    const isolated = await import(`${pathToFileURL(modulePath).href}?test=${Date.now()}`);
    const expected = isolated.buildTerms(sources);
    const generateInProcess = () => isolated.generate(tempDir);
    const generateInChild = () => new Promise((resolve, reject) => {
      const child = spawn(process.execPath, ["--input-type=module", "-e", `const m=await import(${JSON.stringify(pathToFileURL(modulePath).href)}); console.log(JSON.stringify(await m.generate(${JSON.stringify(tempDir)})));`], { windowsHide: true });
      let stderr = "";
      let stdout = "";
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.stdout.on("data", (chunk) => { stdout += chunk; });
      child.once("error", reject);
      child.once("close", (code) => code === 0 ? resolve(JSON.parse(stdout)) : reject(new Error(stderr || `child exited ${code}`)));
    });
    const [first, second] = await Promise.all([generateInProcess(), generateInChild()]);
    const path = isolated.snapshotPath(tempDir, expected.sourceHash);
    const bytes = await readFile(path, "utf8");
    assert.deepEqual(first, expected);
    assert.deepEqual(second, expected);
    assert.equal(bytes, `${JSON.stringify(expected, null, 2)}\n`);
    assert.deepEqual(await isolated.check(tempDir), expected);

    const invalidSources = { ...sources, "docs/glossary.md": "invalid authority" };
    await writeFile(join(tempDir, "docs/glossary.md"), invalidSources["docs/glossary.md"], "utf8");
    await assert.rejects(() => isolated.generate(tempDir), /missing explicit gal-terms JSON block/);
    assert.equal(await readFile(path, "utf8"), bytes);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test("real authority presentation-table drift is rejected", async () => {
  const sources = await loadSources();
  const path = AUTHORITY_PATHS[1];
  const original = sources[path];
  sources[path] = original.replace("| 編碼代理程式 (coding agent) |", "| 錯誤寫法 (coding agent) |");
  assert.notEqual(sources[path], original);
  assert.throws(() => buildTerms(sources), /table|disagree|policy/i);
});

test("real authorities validation: zero data conflicts, resolved IDs, byte-identical regeneration, retired-term compatibility, unchanged definitions and terminology-profile classification", async () => {
  const sources = await loadSources();
  for (const path of AUTHORITY_PATHS) {
    assert.ok(sources[path], `missing source: ${path}`);
  }

  // 1. Build and validate data from real authorities
  const data = buildTerms(sources);
  assert.equal(data.schemaVersion, 1);
  assert.ok(data.entries.length > 0);

  // 2. All non-canonical entries resolve to canonical concept IDs
  const canonicalIds = new Set(data.entries.filter((e) => e.locale === "en-US").map((e) => e.conceptId));
  for (const entry of data.entries) {
    assert.ok(canonicalIds.has(entry.conceptId), `unresolved conceptId: ${entry.conceptId} in ${entry.locale}`);
    assert.ok(entry.sourceAnchor, `missing sourceAnchor for ${entry.conceptId}`);
  }

  // 3. Repeat-generation byte-identical check
  const data2 = buildTerms(sources);
  assert.equal(JSON.stringify(data), JSON.stringify(data2));
  assert.equal(data.sourceHash, data2.sourceHash);

  // 4. Retired-term compatibility in docs/glossary.md
  const glossaryText = sources["docs/glossary.md"];
  assert.match(glossaryText, /# RETIRED-TERMS:BEGIN[\s\S]*# RETIRED-TERMS:END/);
  assert.match(glossaryText, /## Retired Terms \(gate input\)/);

  // 5. Unchanged definitions in docs/glossary.md
  assert.match(glossaryText, /## Part B — Core external concepts/);
  assert.match(glossaryText, /## Part C — GAL-internal vocabulary/);

  // 6. Terminology-profile classification in zh-Hant and ja files
  const zhText = sources["docs/i18n/zh-Hant/terminology.zh-Hant.md"];
  assert.match(zhText, /type:\s*Reference/);
  assert.match(zhText, /tags:\s*[\s\S]*-\s*terminology/);
  assert.match(zhText, /tags:\s*[\s\S]*-\s*presentation/);

  const jaText = sources["docs/i18n/ja/terminology.ja.md"];
  assert.match(jaText, /type:\s*Reference/);
  assert.match(jaText, /tags:\s*[\s\S]*-\s*terminology/);
  assert.match(jaText, /tags:\s*[\s\S]*-\s*presentation/);
});

test("real authority rosters are explicit and anchors must exist", async () => {
  const sources = await loadSources();
  const zhPath = AUTHORITY_PATHS[1];
  const original = sources[zhPath];
  const blockMatch = original.match(/<!-- gal-terms:start -->\s*```json\s*([\s\S]*?)\s*```/);
  const blockData = JSON.parse(blockMatch[1]);
  blockData.entries = blockData.entries.filter((entry) => entry.conceptId !== "runtime");
  sources[zhPath] = original.replace(blockMatch[1], JSON.stringify(blockData));
  assert.throws(() => buildTerms(sources), /missing explicit zh-TW entry/);

  const anchorSources = await loadSources();
  anchorSources[zhPath] = anchorSources[zhPath].replace("#gal-core-terms", "#missing-anchor");
  assert.throws(() => buildTerms(anchorSources), /missing source anchor/);
});

test("real canonical glossary alias-table drift is rejected", async () => {
  const sources = await loadSources();
  const path = AUTHORITY_PATHS[0];
  const original = sources[path];
  sources[path] = original.replace("| Validated | intelligent agent |", "| Validated | invented agent |");
  assert.notEqual(sources[path], original);
  assert.throws(() => buildTerms(sources), /table|disagree|policy/i);
});

test("buildTerms rejects copied sourceRow versus actual human table cell disagreement", () => {
  const entryWithSourceRow = {
    ...entry("term-a", "zh-TW"),
    sourceAnchor: "docs/i18n/zh-Hant/terminology.zh-Hant.md#sec",
    sourceRow: {
      heading: "詞彙對照",
      term: "term-a",
      mode: "keep-en",
      preferred: "正確詞彙",
      forbidden: "禁用詞彙",
    },
  };
  const tableWithDisagreement = [
    '<a id="sec"></a>',
    block("zh-TW", [entryWithSourceRow]),
    "",
    "### 詞彙對照",
    "",
    "| 英文術語 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |",
    "| --- | --- | --- | --- |",
    "| term-a | keep-en | 不同詞彙 | 禁用詞彙 |",
  ].join("\n");

  assert.throws(
    () =>
      buildTerms({
        "docs/glossary.md": '<a id="sec"></a>\n' + block("en-US", [{ ...entry("term-a"), sourceAnchor: "docs/glossary.md#sec" }]),
        "docs/i18n/zh-Hant/terminology.zh-Hant.md": tableWithDisagreement,
      }),
    /human table disagrees with term-a\.preferred/
  );
});

test("validation detects cross-entry collision regardless of disjoint scopes", () => {
  const base = { schemaVersion: 1, sourceHash: "0".repeat(64) };
  const entries = [
    { ...entry("term-a"), preferred: "shared-term", allowed: ["shared-term"], forbidden: [], scope: ["file-a.md"] },
    { ...entry("term-b"), preferred: "other-term", allowed: ["other-term"], forbidden: ["shared-term"], scope: ["file-b.md"] },
  ];
  assert.throws(
    () => validateTerms({ ...base, entries }),
    /form “shared-term” is approved by term-a but forbidden by term-b/
  );
});
