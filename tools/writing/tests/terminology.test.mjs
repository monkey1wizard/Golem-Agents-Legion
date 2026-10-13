import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import nativeTerminology, { createTerminologyRule, loadData, OPERATIONAL_RULE, RULE_ID } from "../rules/terminology.mjs";
import { TextlintKernel } from "@textlint/kernel";
import markdownPlugin from "@textlint/textlint-plugin-markdown";
const terminology = (context, options) => createTerminologyRule(options.data, { ...options, channel: "all" })(context);

test("native diagnostics retain exact ranges, sources and configured hard/advisory severities", async () => {
  const entry = (conceptId, preferred, forbidden, mode) => ({ conceptId, locale: "en-US", preferred, allowed: [preferred], forbidden, mode, scope: ["prose"], exceptions: [], sourceAnchor: "fixture:1" });
  const data = { schemaVersion: 1, sourceHash: "a".repeat(64), entries: [entry("hard-word", "good", ["bad"], "keep-en"), entry("context-word", "contextual", [], "contextual")] };
  const text = "😀 bad and contextual. `bad contextual`";
  const result = await new TextlintKernel().lintText(text, {
    ext: ".md", filePath: "native.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [
      { ruleId: "GAL/terminology-hard", rule: createTerminologyRule(data, { channel: "hard" }), options: { severity: "error" } },
      { ruleId: "GAL/terminology-advisory", rule: createTerminologyRule(data, { channel: "advisory" }), options: { severity: "warning" } },
    ],
  });
  assert.deepEqual(result.messages.map(({ range, severity }) => ({ range, severity })), [{ range: [3, 6], severity: 2 }, { range: [11, 21], severity: 1 }]);
  assert.ok(result.messages.every(({ message }) => message.includes("[source: fixture:1]")));
  assert.deepEqual(result.messages.map(({ data }) => data.suggestion.replacement), ["good", null]);
  for (const finding of result.messages) {
    assert.equal(finding.data.sourceAnchor, "fixture:1");
    assert.equal(finding.data.suggestion.applicability, "manual");
    assert.match(finding.data.suggestion.text, /preserve the intended meaning/);
    assert.equal(finding.fix, undefined);
  }
  assert.throws(() => nativeTerminology({}, { data, testOnly: true }), /not a production option/);
});

test("native reported objects preserve configured warning severity for hard-channel findings", async () => {
  const data = { schemaVersion: 1, sourceHash: "a".repeat(64), entries: [{ conceptId: "word", locale: "en-US", preferred: "good", allowed: ["good"], forbidden: ["bad"], mode: "keep-en", scope: ["prose"], exceptions: [], sourceAnchor: "fixture:1" }] };
  const result = await new TextlintKernel().lintText("bad", {
    ext: ".md", filePath: "native.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [{ ruleId: "GAL/terminology", rule: createTerminologyRule(data, { channel: "hard" }), options: { severity: "warning" } }],
  });
  assert.equal(result.messages.length, 1);
  assert.equal(result.messages[0].severity, 1);
  assert.match(result.messages[0].message, /\[advisory\]/);
  assert.equal(result.messages[0].data.suggestion.replacement, "good");
});
import { buildTerms, loadSources, snapshotPath, generate } from "../terms.mjs";

function createMockContext(source, filePath = "fixture.md") {
  const reports = [];
  return {
    context: {
      Syntax: { Document: "Document" },
      getSource: () => source,
      getFilePath: () => filePath,
      report: (_node, finding) => reports.push(finding),
    },
    reports,
  };
}

const sampleData = {
  schemaVersion: 1,
  sourceHash: "0".repeat(64),
  entries: [
    {
      conceptId: "word",
      locale: "en-US",
      preferred: "preferred-term",
      allowed: ["preferred-term", "ok-term"],
      forbidden: ["bad-term"],
      mode: "keep-en",
      scope: ["prose"],
      exceptions: ["allow-bad-term-here"],
      sourceAnchor: "fixture:10",
    },
    {
      conceptId: "coding-agent",
      locale: "en-US",
      preferred: "coding agent",
      allowed: ["coding agent"],
      forbidden: ["programming agent"],
      mode: "keep-en",
      scope: ["prose"],
      exceptions: [],
      sourceAnchor: "fixture:15",
    },
    {
      conceptId: "coding-agent",
      locale: "zh-TW",
      preferred: "編碼代理程式",
      allowed: ["編碼代理程式 (coding agent)"],
      forbidden: ["代碼代理"],
      mode: "bilingual-first-use",
      scope: ["prose"],
      exceptions: [],
      sourceAnchor: "fixture:20",
    },
    {
      conceptId: "status-concept",
      locale: "en-US",
      preferred: "state",
      allowed: ["state"],
      forbidden: ["status-bad"],
      mode: "localized",
      scope: ["prose"],
      exceptions: [],
      sourceAnchor: "fixture:30",
    },
    {
      conceptId: "translit-concept",
      locale: "en-US",
      preferred: "translit",
      allowed: ["translit"],
      forbidden: ["bad-translit"],
      mode: "transliterated",
      scope: ["prose"],
      exceptions: [],
      sourceAnchor: "fixture:40",
    },
    {
      conceptId: "context-concept",
      locale: "en-US",
      preferred: "contextual-word",
      allowed: ["contextual-word"],
      forbidden: ["bad-context"],
      mode: "contextual",
      scope: ["prose"],
      exceptions: [],
      sourceAnchor: "fixture:50",
    },
  ],
};

test("terminology rule has a fail-closed operational loader", async () => {
  // Invalid data objects fail without relying on a pre-generated root cache.
  assert.throws(
    () => createTerminologyRule({ schemaVersion: 2, entries: [] }),
    (err) => err.message.includes("schemaVersion=1")
  );

  const sourceRoot = process.cwd();
  const fixtureRoot = mkdtempSync(join(tmpdir(), "gal-rule-terms-"));
  const sources = await loadSources(sourceRoot);
  for (const [path, content] of Object.entries(sources)) {
    const target = join(fixtureRoot, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content, "utf8");
  }
  const expected = buildTerms(sources);
  const derivedPath = snapshotPath(fixtureRoot, expected.sourceHash);
  const oldPin = process.env.GAL_WRITING_TERMS_HASH;
  const oldCwd = process.cwd();
  process.chdir(fixtureRoot);
  process.env.GAL_WRITING_TERMS_HASH = expected.sourceHash;
  try {
    await generate(fixtureRoot);
    const loaded = loadData();
    assert.ok(loaded.entries.length > 0);
    const originalContent = readFileSync(derivedPath, "utf8");
    unlinkSync(derivedPath);
    assert.throws(() => loadData(), (err) => err.ruleId === OPERATIONAL_RULE && err.severity === 2);
    writeFileSync(derivedPath, originalContent, "utf8");
    let corrupted = JSON.parse(originalContent);
    corrupted.sourceHash = "0".repeat(64);
    writeFileSync(derivedPath, JSON.stringify(corrupted), "utf8");
    assert.throws(() => loadData(), (err) => err.ruleId === OPERATIONAL_RULE && err.severity === 2);
    corrupted = JSON.parse(originalContent);
    corrupted.entries[0].preferred = `${corrupted.entries[0].preferred} altered`;
    writeFileSync(derivedPath, JSON.stringify(corrupted), "utf8");
    assert.throws(() => loadData(), (err) => err.ruleId === OPERATIONAL_RULE && err.severity === 2);
  } finally {
    if (oldPin === undefined) delete process.env.GAL_WRITING_TERMS_HASH;
    else process.env.GAL_WRITING_TERMS_HASH = oldPin;
    process.chdir(oldCwd);
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("loader detects parent-child source drift when both snapshots present and rejects malformed pins", async () => {
  const sourceRoot = process.cwd();
  const fixtureRoot = mkdtempSync(join(tmpdir(), "gal-loader-drift-"));
  const sourcesA = await loadSources(sourceRoot);
  for (const [path, content] of Object.entries(sourcesA)) {
    const target = join(fixtureRoot, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content, "utf8");
  }
  const termsA = buildTerms(sourcesA);
  const hashA = termsA.sourceHash;
  await generate(fixtureRoot);

  // Now create sources B by appending a newline to glossary in fixtureRoot
  const glossaryPath = join(fixtureRoot, "docs", "glossary.md");
  writeFileSync(glossaryPath, readFileSync(glossaryPath, "utf8") + "\n", "utf8");
  const sourcesB = await loadSources(fixtureRoot);
  const termsB = buildTerms(sourcesB);
  const hashB = termsB.sourceHash;
  assert.notEqual(hashA, hashB);
  await generate(fixtureRoot);

  // Both snapshot files are present on disk
  const pathA = snapshotPath(fixtureRoot, hashA);
  const pathB = snapshotPath(fixtureRoot, hashB);
  assert.ok(readFileSync(pathA, "utf8"));
  assert.ok(readFileSync(pathB, "utf8"));

  const oldPin = process.env.GAL_WRITING_TERMS_HASH;
  const oldCwd = process.cwd();
  process.chdir(fixtureRoot);
  try {
    // 1. Parent pin A vs current sources B: fails operationally even though both snapshots exist
    process.env.GAL_WRITING_TERMS_HASH = hashA;
    assert.throws(
      () => loadData(),
      (err) => err.ruleId === OPERATIONAL_RULE && err.message.includes("does not match parent pin")
    );

    // 2. Malformed direct pin: fails operationally
    process.env.GAL_WRITING_TERMS_HASH = "malformed-direct-pin-000";
    assert.throws(
      () => loadData(),
      (err) => err.ruleId === OPERATIONAL_RULE && err.message.includes("does not match parent pin")
    );

    // 3. Matching pin B: succeeds
    process.env.GAL_WRITING_TERMS_HASH = hashB;
    const loaded = loadData();
    assert.equal(loaded.sourceHash, hashB);

    // 4. Unpinned: derives hash B and succeeds
    delete process.env.GAL_WRITING_TERMS_HASH;
    const unpinned = loadData();
    assert.equal(unpinned.sourceHash, hashB);
  } finally {
    if (oldPin === undefined) delete process.env.GAL_WRITING_TERMS_HASH;
    else process.env.GAL_WRITING_TERMS_HASH = oldPin;
    process.chdir(oldCwd);
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("C-01: forbidden forms in ordinary prose reported, protected content and exceptions excluded", async () => {
  const source = [
    "# Heading with bad-term in prose",
    "",
    "Here is `bad-term` in inline code.",
    "",
    "```javascript",
    "const bad-term = true;",
    "```",
    "",
    "Here is an exception: allow-bad-term-here in prose.",
    "",
    "And here is another forbidden bad-term in plain prose.",
  ].join("\n");

  const { context, reports } = createMockContext(source);
  const rule = terminology(context, { data: sampleData, testOnly: true, locale: "en-US" });
  await rule.Document({});

  // Only the two occurrences in unexcepted prose should be reported
  assert.equal(reports.length, 2);
  for (const report of reports) {
    assert.equal(report.ruleId, RULE_ID);
    assert.equal(report.severity, 2);
    assert.equal(report.sourceAnchor, "fixture:10");
    assert.match(report.message, /Forbidden form “bad-term”/);
    assert.equal(report.fix, undefined); // read-only default
  }
});

test("C-02 & C-06: approved vs forbidden forms and document-level first-use", async () => {
  // Test case 1: correct bilingual first-use
  const validSource = "我們推薦使用編碼代理程式 (coding agent)，後續可稱編碼代理程式。";
  const { context: validCtx, reports: validReports } = createMockContext(validSource);
  const validRule = terminology(validCtx, { data: sampleData, testOnly: true, locale: "zh-TW" });
  await validRule.Document({});
  assert.equal(validReports.length, 0);

  // Test case 2: missing bilingual on first use yields advisory warning
  const missingBilingualSource = "我們推薦使用編碼代理程式進行開發。";
  const { context: warnCtx, reports: warnReports } = createMockContext(missingBilingualSource);
  const warnRule = terminology(warnCtx, { data: sampleData, testOnly: true, locale: "zh-TW" });
  await warnRule.Document({});
  assert.equal(warnReports.length, 1);
  assert.equal(warnReports[0].severity, 1); // advisory
  assert.match(warnReports[0].message, /First use of coding-agent must use/);

  // Test case 3: unapproved alias triggers forbidden form error
  const forbiddenSource = "這是一個代碼代理工具。";
  const { context: errCtx, reports: errReports } = createMockContext(forbiddenSource);
  const errRule = terminology(errCtx, { data: sampleData, testOnly: true, locale: "zh-TW" });
  await errRule.Document({});
  assert.equal(errReports.length, 1);
  assert.equal(errReports[0].severity, 2);
  assert.match(errReports[0].message, /Forbidden form “代碼代理”/);
});

test("C-11: presentation modes supported and contextual forms yield advice", async () => {
  const source = "Please check this contextual-word in ordinary prose.";
  const { context, reports } = createMockContext(source);
  const rule = terminology(context, { data: sampleData, testOnly: true, locale: "en-US" });
  await rule.Document({});

  assert.equal(reports.length, 1);
  assert.equal(reports[0].ruleId, RULE_ID);
  assert.equal(reports[0].severity, 1); // advisory
  assert.match(reports[0].message, /Context determines the presentation of context-concept/);
});

test("forbidden matching preserves complete approved phrases, repeats, scope, exceptions, and protected code", async () => {
  const data = {
    schemaVersion: 1,
    sourceHash: "b".repeat(64),
    entries: [
      { conceptId: "bad", locale: "en-US", preferred: "good phrase", allowed: ["good phrase"], forbidden: ["bad"], mode: "keep-en", scope: ["prose"], exceptions: ["allow bad"], sourceAnchor: "fixture:60" },
      { conceptId: "outside", locale: "en-US", preferred: "outside", allowed: ["outside"], forbidden: ["bad"], mode: "keep-en", scope: ["other.md"], exceptions: [], sourceAnchor: "fixture:61" },
    ],
  };
  const source = "bad good phrase bad bad allow bad\n\n`bad`\n\n```text\nbad\n```";
  const { context, reports } = createMockContext(source, "fixture.md");
  await terminology(context, { data, testOnly: true, locale: "en-US" }).Document({});
  assert.deepEqual(reports.map(({ index, severity }) => ({ index, severity })), [{ index: 0, severity: 2 }, { index: 16, severity: 2 }, { index: 20, severity: 2 }]);
});

for (const [locale, text] of [
  ["en-US", "An AI agent uses MCP (Model Context Protocol)."],
  ["zh-TW", "編碼代理程式 (coding agent) 與 AI 代理程式 (AI agent)。"],
]) {
  test(`real authority approved prose has no hard findings: ${locale}`, async () => {
    const result = await new TextlintKernel().lintText(text, {
      ext: ".md", filePath: "approved-prose.md",
      plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
      rules: [{ ruleId: "GAL/terminology-hard", rule: nativeTerminology, options: { locale, channel: "hard", severity: "error" } }],
    });
    assert.deepEqual(result.messages.filter(({ severity }) => severity === 2), [], "Authority-approved terms must not produce hard findings");
  });
}

test("native real-authority mixed prose retains only standalone forbidden findings", async () => {
  const text = "編碼代理程式 (coding agent) 與 AI 代理程式 (AI agent)。代碼代理，再次代碼代理。 `代碼代理`";
  const result = await new TextlintKernel().lintText(text, {
    ext: ".md", filePath: "mixed-prose.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [{ ruleId: "GAL/terminology-hard", rule: nativeTerminology, options: { locale: "zh-TW", channel: "hard", severity: "error" } }],
  });
  const hard = result.messages.filter(({ severity }) => severity === 2);
  const first = text.indexOf("代碼代理");
  const second = text.indexOf("代碼代理", first + 4);
  assert.ok(hard.some(({ range }) => range[0] === first && range[1] === first + 4));
  assert.ok(hard.some(({ range }) => range[0] === second && range[1] === second + 4));
  assert.ok(hard.every(({ range }) => (range[0] >= first && range[1] <= first + 4) || (range[0] >= second && range[1] <= second + 4)), "Legal words and inline code must stay clean");
});

test("native English authority prose keeps generic mapping, status, and credentials legal", async () => {
  const text = [
    "A profile is not a translation of this glossary. It omits the naming logic rules, complete definition tables, and retired-term gate blocks, mapping only canonical English headwords to their presentation mode.",
    "Key-value map of credentials (such as API keys) using UPPER_SNAKE_CASE keys.",
    "The current status is stable.",
  ].join(" ");
  const result = await new TextlintKernel().lintText(text, {
    ext: ".md", filePath: "real-authority-regression.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [{ ruleId: "GAL/terminology-hard", rule: nativeTerminology, options: { locale: "en-US", channel: "hard", severity: "error" } }],
  });
  assert.deepEqual(result.messages.filter(({ severity }) => severity === 2), [], "Generic English words must remain legal outside a proven terminology alias");
});

test("native English retired terminology remains a hard finding", async () => {
  const retiredForm = ["MCP", "provider"].join(" ");
  const text = `Use ${retiredForm} only when the retired role is being discussed.`;
  const result = await new TextlintKernel().lintText(text, {
    ext: ".md", filePath: "explicit-alias-regression.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [{ ruleId: "GAL/terminology-hard", rule: nativeTerminology, options: { locale: "en-US", channel: "hard", severity: "error" } }],
  });
  const hard = result.messages.filter(({ severity }) => severity === 2);
  assert.equal(hard.length, 1);
  assert.ok(hard[0].message.includes(retiredForm));
});

test("native general English words do not imply misuse of GAL concepts", async () => {
  const text = "The distribution includes a waterfall example and a personal improvement guide.";
  const result = await new TextlintKernel().lintText(text, {
    ext: ".md", filePath: "ordinary-prose.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [{ ruleId: "GAL/terminology-hard", rule: nativeTerminology, options: { locale: "en-US", channel: "hard", severity: "error" } }],
  });
  assert.deepEqual(result.messages.filter(({ severity }) => severity === 2), [], "Ordinary English words cannot prove a GAL concept naming error");
});
