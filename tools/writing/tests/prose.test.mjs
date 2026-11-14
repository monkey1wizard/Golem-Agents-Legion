import test from "node:test";
import assert from "node:assert/strict";
import { TextlintKernel } from "@textlint/kernel";
import markdownPlugin from "@textlint/textlint-plugin-markdown";
import { Syntax } from "@textlint/markdown-to-ast";
import { proseRanges, filter, utf16ToUtf8Offset, utf8ToUtf16Offset } from "../prose.mjs";

const slices = (text, ranges) => ranges.map(([start, end]) => text.slice(start, end));

test("preserves UTF-16 source offsets across CJK, emoji, and CRLF", () => {
  const text = "說明😀\r\n\r\n`命令😀`\r\n\r\n普通文字。";
  const ranges = proseRanges(text, "notes.md");
  assert.deepEqual(slices(text, ranges), ["說明😀", "普通文字。"]);
  assert.deepEqual(ranges, [[0, 4], [18, 23]]);
  const testOffsets = [0, 2, 4, 6, 8, 18, 23];
  for (const offset of testOffsets) {
    const byteOffset = utf16ToUtf8Offset(text, offset);
    assert.equal(utf8ToUtf16Offset(text, byteOffset), offset, `Round-trip conversion failed at offset ${offset}`);
  }
  assert.equal(utf16ToUtf8Offset(text, 2), 6);
  assert.equal(utf16ToUtf8Offset(text, 4), 10);
  assert.equal(utf16ToUtf8Offset(text, 6), 12);
});

test("protects frontmatter, code, literals, destinations, machine text, and quotations", () => {
  const text = [
    "---",
    "title: do not lint",
    "---",
    "Readable [label](https://example.test/target) prose.",
    "",
    "```sh",
    "node tools/writing/check.mjs --required",
    "```",
    "",
    "> Verbatim quotation.",
    "",
    "Use https://example.test/a and `literal` in prose.",
    "<!-- machine anchor -->",
    "receipt: preserved",
  ].join("\n");
  assert.deepEqual(slices(text, proseRanges(text, "notes.md")), ["Readable ", "label", " prose.", "Use ", " and ", " in prose."]);
});

test("protects complete paths and literal tokens without consuming following prose", () => {
  const text = "Run node tools/writing/check.mjs --required then lint this prose. Use ID-42 and `literal` before ordinary quoted layout.";
  assert.deepEqual(
    slices(text, proseRanges(text, "notes.md")),
    ["Run ", " ", " ", " then lint this prose. Use ", " and ", " before ordinary quoted layout."]
  );
});

test("scans ordinary table prose and link labels, excluding designated cells", () => {
  const text = [
    "| Term | Example | Notes |",
    "| --- | --- | --- |",
    "| preferred | `literal` | This prose is lintable. |",
    "| accepted | sample | Another note. |",
  ].join("\n");
  assert.deepEqual(slices(text, proseRanges(text, "table.md")), ["Notes", "This prose is lintable.", "Another note."]);
});

test("protects exact English, zh-TW, and ja-JP forbidden-form columns while preserving adjacent prose offsets", () => {
  const text = [
    "| Forbidden | Notes |", "| --- | --- |", "| sentinel English forbidden form | sentinel English note |",
    "",
    "| 禁止寫法 | 備註 |", "| --- | --- |", "| sentinel zh forbidden form | sentinel zh note |",
    "",
    "| 避ける表記 | 説明 |", "| --- | --- |", "| sentinel ja forbidden form | sentinel ja note |",
    "", "See [sentinel link label](https://example.test/target).",
  ].join("\n");
  const ranges = proseRanges(text, "localized-tables.md");
  const prose = slices(text, ranges);
  for (const value of ["sentinel English forbidden form", "sentinel zh forbidden form", "sentinel ja forbidden form"]) {
    assert.equal(prose.some((slice) => slice.includes(value)), false, `${value} must be protected`);
  }
  for (const value of ["sentinel English note", "sentinel zh note", "sentinel ja note", "sentinel link label"]) {
    assert.ok(prose.some((slice) => slice.includes(value)), `${value} must remain lintable`);
    const start = text.indexOf(value);
    assert.ok(ranges.some(([rangeStart, rangeEnd]) => rangeStart <= start && rangeEnd >= start + value.length), `${value} must retain its original source range`);
  }
});

test("filter uses the same ranges and the public shouldIgnore contract", () => {
  const text = "Lint this. `do not lint` and https://example.test remain protected.";
  const ignored = [];
  const handlers = filter({
    getSource: () => text,
    getFilePath: () => "notes.md",
    shouldIgnore: (range, optional) => ignored.push({ range, optional }),
  });
  handlers.Document({});
  assert.ok(ignored.some(({ range }) => text.slice(range[0], range[1]).includes("do not lint")));
  assert.ok(ignored.some(({ range }) => text.slice(range[0], range[1]).includes("https://example.test")));
  assert.equal(ignored.some(({ range }) => text.slice(range[0], range[1]).includes("Lint this.")), false);
});

test("proves native filter suppresses diagnostics in protected spans and keeps ordinary table prose and link labels visible", () => {
  const tableDoc = [
    "| Term | Example | Notes |",
    "| --- | --- | --- |",
    "| forbidden_term | sample_forbidden | visible error in table prose |",
  ].join("\n");

  const tableIgnored = [];
  const tableHandlers = filter({
    getSource: () => tableDoc,
    getFilePath: () => "table.md",
    shouldIgnore: (range) => tableIgnored.push(range),
  });
  tableHandlers.Document({});

  const termIdx = tableDoc.indexOf("forbidden_term");
  const termRange = [termIdx, termIdx + "forbidden_term".length];
  assert.ok(
    tableIgnored.some(([start, end]) => termRange[0] >= start && termRange[1] <= end),
    "Diagnostic in Term column should be suppressed by filter"
  );

  const exampleIdx = tableDoc.indexOf("sample_forbidden");
  const exampleRange = [exampleIdx, exampleIdx + "sample_forbidden".length];
  assert.ok(
    tableIgnored.some(([start, end]) => exampleRange[0] >= start && exampleRange[1] <= end),
    "Diagnostic in Example column should be suppressed by filter"
  );

  const notesIdx = tableDoc.indexOf("visible error in table prose");
  const notesRange = [notesIdx, notesIdx + "visible error in table prose".length];
  assert.equal(
    tableIgnored.some(([start, end]) => Math.max(notesRange[0], start) < Math.min(notesRange[1], end)),
    false,
    "Diagnostic in ordinary table prose must remain visible (not ignored)"
  );

  const linkDoc = "Check [violation in link label](https://example.test/protected_destination).";
  const linkIgnored = [];
  const linkHandlers = filter({
    getSource: () => linkDoc,
    getFilePath: () => "link.md",
    shouldIgnore: (range) => linkIgnored.push(range),
  });
  linkHandlers.Document({});

  const destIdx = linkDoc.indexOf("https://example.test/protected_destination");
  const destRange = [destIdx, destIdx + "https://example.test/protected_destination".length];
  assert.ok(
    linkIgnored.some(([start, end]) => destRange[0] >= start && destRange[1] <= end),
    "Diagnostic in link destination should be suppressed by filter"
  );

  const labelIdx = linkDoc.indexOf("violation in link label");
  const labelRange = [labelIdx, labelIdx + "violation in link label".length];
  assert.equal(
    linkIgnored.some(([start, end]) => Math.max(labelRange[0], start) < Math.min(labelRange[1], end)),
    false,
    "Diagnostic in link label must remain visible (not ignored)"
  );
});

test("proves the native textlint kernel suppresses only protected diagnostics", async () => {
  const nativeDoc = "sentinel prose. `sentinel literal` and https://example.test/sentinel stay protected.\n\n| Term | Notes |\n| --- | --- |\n| sentinel_term | sentinel table prose |\n\nCheck [sentinel link label](https://example.test/sentinel).";
  const sentinelRule = {
    ruleId: "sentinel",
    rule: (context) => ({
      [Syntax.Document](node) {
        for (const match of context.getSource(node).matchAll(/sentinel/g)) {
          context.report(node, new context.RuleError("sentinel", { index: match.index }));
        }
      },
    }),
  };
  const nativeResult = await new TextlintKernel().lintText(nativeDoc, {
    ext: ".md",
    filePath: "native.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [sentinelRule],
    filterRules: [{ ruleId: "prose-filter", rule: filter }],
  });
  const nativeMessages = nativeResult.messages ?? [];
  assert.deepEqual(nativeMessages.map(({ index }) => index), [0, nativeDoc.indexOf("sentinel table prose"), nativeDoc.indexOf("sentinel link label")]);
});

test("retains prose after an identifier, command name, and relative path", () => {
  const identifier = ["T", "99"].join("-");
  const text = `${identifier} has ordinary prose. Use git and ordinary prose.`;
  assert.deepEqual(slices(text, proseRanges(text)), [" has ordinary prose. Use ", " and ordinary prose."]);

  const gitText = "Use git and forbidden prose.";
  assert.deepEqual(slices(gitText, proseRanges(gitText)), ["Use ", " and forbidden prose."]);

  const idText = `${identifier} has forbidden prose.`;
  assert.deepEqual(slices(idText, proseRanges(idText)), [" has forbidden prose."]);

  const relPathText = "tools/writing/file.mjs has ordinary prose.";
  assert.deepEqual(slices(relPathText, proseRanges(relPathText)), [" has ordinary prose."]);
});

test("proves native textlint kernel suppresses diagnostics inside relative paths and task IDs while keeping surrounding prose diagnostics", async () => {
  const identifier = ["T", "99"].join("-");
  const doc = `sentinel start tools/writing/sentinel.mjs sentinel middle ${identifier} sentinel end`;
  const sentinelRule = {
    ruleId: "sentinel",
    rule: (context) => ({
      [Syntax.Document](node) {
        for (const match of context.getSource(node).matchAll(/sentinel/g)) {
          context.report(node, new context.RuleError("sentinel", { index: match.index }));
        }
      },
    }),
  };
  const result = await new TextlintKernel().lintText(doc, {
    ext: ".md",
    filePath: "relative_path.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [sentinelRule],
    filterRules: [{ ruleId: "prose-filter", rule: filter }],
  });
  const messages = result.messages ?? [];
  assert.deepEqual(
    messages.map(({ index }) => index),
    [0, doc.indexOf("sentinel middle"), doc.indexOf("sentinel end")]
  );
});

test("native textlint filter protects English and localized forbidden-form columns only", async () => {
  const doc = [
    "| Forbidden | Notes |", "| --- | --- |", "| sentinel_en_forbidden | sentinel_en_note |",
    "",
    "| 禁止寫法 | 備註 |", "| --- | --- |", "| sentinel_zh_forbidden | sentinel_zh_note |",
    "",
    "| 避ける表記 | 説明 |", "| --- | --- |", "| sentinel_ja_forbidden | sentinel_ja_note |",
    "", "[sentinel_link_label](https://example.test/target)",
  ].join("\n");
  const sentinelRule = {
    ruleId: "sentinel",
    rule: (context) => ({
      [Syntax.Document](node) {
        for (const match of context.getSource(node).matchAll(/sentinel_\w+/g)) {
          context.report(node, new context.RuleError("sentinel", { index: match.index }));
        }
      },
    }),
  };
  const result = await new TextlintKernel().lintText(doc, {
    ext: ".md", filePath: "localized-tables.md",
    plugins: [{ pluginId: "markdown", plugin: markdownPlugin.default }],
    rules: [sentinelRule], filterRules: [{ ruleId: "prose-filter", rule: filter }],
  });
  const expected = ["sentinel_en_note", "sentinel_zh_note", "sentinel_ja_note", "sentinel_link_label"]
    .map((value) => doc.indexOf(value));
  assert.deepEqual((result.messages ?? []).map(({ index }) => index), expected);
});
