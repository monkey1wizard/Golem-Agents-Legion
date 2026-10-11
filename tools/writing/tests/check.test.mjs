import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFile, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { test } from "node:test";
import { Client } from "../node_modules/@modelcontextprotocol/sdk/dist/esm/client/index.js";
import { CallToolResultSchema } from "../node_modules/@modelcontextprotocol/sdk/dist/esm/types.js";
import { StdioClientTransport } from "../node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js";
import { buildTerms, generate, loadSources, snapshotPath } from "../terms.mjs";

const root = process.cwd();
const checker = join(root, "tools", "writing", "check.mjs");

function run(args, cwd = root, env = process.env) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [checker, ...args], { cwd, windowsHide: true, env });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("close", (code) => resolve({ code, stdout, stderr }));
  });
}

function runNative(config, file, cwd = root, rulesBaseDir = join(root, "tools", "writing", "node_modules"), env = process.env) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [
      join(root, "tools", "writing", "node_modules", "textlint", "bin", "textlint.js"),
      "--config", config.includes("/") || config.includes("\\") ? config : join(root, "tools", "writing", "config", config),
      "--rules-base-directory", rulesBaseDir,
      "--format", "json",
      file,
    ], { cwd, windowsHide: true, env });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("close", (code) => resolve({ code, stdout, stderr }));
  });
}

function reportFrom(stderr) {
  const match = stderr.match(/report: (.+)\r?\n/);
  assert.ok(match, `report path missing in stderr: ${stderr}`);
  return match[1].trim();
}

async function runMcp(config, text, stdinFilename = "draft.md", cwd = root, rulesBaseDir = join(root, "tools", "writing", "node_modules"), env = process.env, operationalError = false) {
  const configPath = config.includes("/") || config.includes("\\") ? config : join(root, "tools", "writing", "config", config);
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [
      join(root, "tools", "writing", "node_modules", "textlint", "bin", "textlint.js"),
      "--mcp",
      "--config", configPath,
      "--rules-base-directory", rulesBaseDir,
    ],
    cwd,
    env,
    stderr: "pipe",
  });
  const client = new Client({ name: "writing-check-tests", version: "1.0.0" });
  try {
    await client.connect(transport);
    const tools = await client.listTools();
    assert.ok(tools.tools.some((tool) => tool.name === "lintText"));
    const params = { name: "lintText", arguments: { text, stdinFilename } };
    if (operationalError) {
      // textlint 15.2.3 returns an error object that does not satisfy its own
      // lintText output schema. Verify the SDK rejects it, then inspect the
      // protocol result separately so the actual operational cause is checked.
      await assert.rejects(client.callTool(params), (error) => error.code === -32602 && /output schema/.test(error.message));
      const result = await client.request({ method: "tools/call", params }, CallToolResultSchema);
      assert.equal(result.isError, true);
      const payload = JSON.parse(result.content.find((item) => item.type === "text").text);
      assert.equal(payload.type, "lintText_error");
      assert.match(payload.error, /terminology data unavailable or invalid/);
      return result;
    }
    return await client.callTool(params);
  } finally {
    await client.close().catch(() => {});
    await transport.close().catch(() => {});
  }
}

test("rejects an unknown explicit locale with infrastructure exit 2", async () => {
  const result = await run(["--locale", "fr-FR", "--files", "README.md"]);
  assert.equal(result.code, 2);
  assert.match(result.stderr, /unsupported locale/);
});

test("rejects unsupported or missing transport values operationally", async () => {
  for (const value of ["", "unknown", "--required"]) {
    const result = await run(["--transport", value, "--files", "README.md"]);
    assert.equal(result.code, 2, result.stderr);
    assert.match(result.stderr, /--transport must be cli or mcp/);
  }
});

test("official MCP transport produces the same complete GAL diagnostics and provenance as CLI", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-mcp-report-"));
  try {
    const cases = [
      ["en-US", `The ${["MCP", "provider"].join(" ")} was generated in order to help the coding agent.\n`],
      ["zh-TW", "我們推薦使用編碼代理程式。\n"],
      ["ja-JP", `${"あ".repeat(100)}。ないわけではない。提供元を使います。\n`],
      ["en-US", ""],
    ];
    for (const [locale, text] of cases) {
      const file = join(directory, `${locale}.md`);
      await writeFile(file, text, "utf8");
      const reports = [];
      const exits = [];
      for (const transport of ["cli", "mcp"]) {
        const result = await run(["--transport", transport, "--locale", locale, "--files", file], root, { ...process.env, GAL_WRITING_TERMS_HASH: "malformed-inherited-pin" });
        assert.ok([0, 1].includes(result.code), result.stderr);
        exits.push(result.code);
        const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
        reports.push(report);
        assert.equal(report.transport, transport);
        assert.equal(report.completed, true);
        assert.equal(report.versions.textlint, "15.2.3");
        assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(text).digest("hex"));
        assert.equal(report.selectedFiles[0].reason, "explicit override");
        assert.match(report.configHashes[locale], /^[a-f0-9]{64}$/);
        assert.match(report.terms.sourceHash, /^[a-f0-9]{64}$/);
        assert.deepEqual(report.skippedFiles, []);
        if (transport === "mcp") {
          assert.equal(report.servers.length, 1);
          assert.equal(report.servers[0].version, "15.2.3");
        }
        for (const finding of report.findings) {
          assert.equal(finding.engine, "textlint");
          assert.ok(finding.ruleId && finding.rationale && finding.policySource);
          assert.equal(finding.suggestion.applicability, "manual");
          assert.deepEqual(finding.sourceRange, finding.range);
        }
      }
      assert.equal(exits[0], exits[1]);
      assert.deepEqual(reports[0].findings, reports[1].findings);
      assert.deepEqual(await readFile(file), Buffer.from(text));
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("MCP operational failure retains negotiated server and input provenance", async () => {
  const overlay = await mkdtemp(join(tmpdir(), "gal-mcp-failure-"));
  const writing = join(overlay, "tools/writing");
  try {
    for (const file of ["tools/writing/check.mjs", "tools/writing/terms.mjs", "docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await mkdir(dirname(join(overlay, file)), { recursive: true });
      await copyFile(join(root, file), join(overlay, file));
    }
    await symlink(join(root, "tools/writing/node_modules"), join(writing, "node_modules"), "junction");
    await mkdir(join(writing, "config"), { recursive: true });
    await writeFile(join(writing, "broken.mjs"), 'export default (context) => ({[context.Syntax.Document]() { throw new Error("deliberate operational failure"); }});', "utf8");
    await writeFile(join(writing, "config/en-US.json"), JSON.stringify({ rules: { "../broken.mjs": true } }), "utf8");
    const text = "A complete input.\n";
    await writeFile(join(overlay, "sample.md"), text, "utf8");
    const result = await new Promise((resolvePromise, reject) => {
      const child = spawn(process.execPath, [join(writing, "check.mjs"), "--transport", "mcp", "--locale", "en-US", "--files", "sample.md"], { cwd: overlay, windowsHide: true });
      let stderr = "";
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.once("error", reject);
      child.once("close", (code) => resolvePromise({ code, stderr }));
    });
    assert.equal(result.code, 2, result.stderr);
    const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.equal(report.completed, false);
    assert.equal(report.transport, "mcp");
    assert.equal(report.servers[0].version, "15.2.3");
    assert.equal(report.versions.textlint, "15.2.3");
    assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(text).digest("hex"));
    assert.match(report.error.message, /textlint MCP failed/);
    assert.match(report.configHashes["en-US"], /^[a-f0-9]{64}$/);
  } finally { await rm(overlay, { recursive: true, force: true }); }
});

test("rejects empty or missing explicit locale values without fallback", async () => {
  const empty = await run(["--locale", "", "--files", "README.md"]);
  assert.equal(empty.code, 2, empty.stderr);
  assert.match(empty.stderr, /unsupported locale/);

  const missing = await run(["--locale", "--files", "README.md"]);
  assert.equal(missing.code, 2, missing.stderr);
  assert.match(missing.stderr, /--locale requires/);

  const required = await run(["--required", "--locale", ""]);
  assert.equal(required.code, 2, required.stderr);
  assert.match(required.stderr, /cannot be combined/);
});

test("resolves locale by override, frontmatter, then managed path and reports provenance", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-locale-resolution-"));
  try {
    const file = join(directory, "README.md");
    await writeFile(file, "---\nlang: ja\n---\nGAL は完了しました。\n", "utf8");
    let result = await run(["--files", file]);
    assert.equal(result.code, 0, result.stderr);
    let report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.equal(report.selectedFiles[0].locale, "ja-JP");
    assert.equal(report.selectedFiles[0].reason, "frontmatter lang");
    result = await run(["--locale", "en-US", "--files", file]);
    assert.ok([0, 1].includes(result.code), result.stderr);
    report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.equal(report.selectedFiles[0].locale, "en-US");
    assert.equal(report.selectedFiles[0].reason, "explicit override");
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("fails closed for invalid lang and unknown path without override", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-locale-invalid-"));
  try {
    const file = join(directory, "README.md");
    await writeFile(file, "---\nlang: fr-FR\n---\nText.\n", "utf8");
    const invalid = await run(["--files", file]);
    assert.equal(invalid.code, 2);
    assert.match(invalid.stderr, /unsupported frontmatter lang/);
    const unknown = join(directory, "draft.md");
    await writeFile(unknown, "Text.\n", "utf8");
    const unresolved = await run(["--files", unknown]);
    assert.equal(unresolved.code, 2);
    assert.match(unresolved.stderr, /unknown path/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("locale boundaries, aliases, malformed metadata precedence, and input preservation", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-locale-boundaries-"));
  try {
    for (const auth of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await mkdir(dirname(join(directory, auth)), { recursive: true });
      await copyFile(join(root, auth), join(directory, auth));
    }
    const check = async (relative) => {
      const result = await run(["--files", relative], directory);
      if (result.stderr.includes("report: ")) {
        const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
        return { result, report };
      }
      return { result, report: null };
    };
    const managedZh = "docs/i18n/zh-Hant/guide.md";
    await writeFile(join(directory, managedZh), "台灣繁體內容。\n", "utf8");
    let { result, report } = await check(managedZh);
    assert.equal(result.code, 0, result.stderr);
    assert.equal(report.selectedFiles[0].locale, "zh-TW");
    assert.equal(report.selectedFiles[0].reason, "managed path");

    const managedZhMetadata = "docs/i18n/zh-Hant/lang-guide.md";
    const managedZhBytes = "---\nlang: zh-Hant\n---\n繁體內容。\n";
    await writeFile(join(directory, managedZhMetadata), managedZhBytes, "utf8");
    ({ result, report } = await check(managedZhMetadata));
    assert.ok([0, 1].includes(result.code), result.stderr);
    assert.equal(report.selectedFiles[0].locale, "zh-TW");
    assert.equal(report.selectedFiles[0].reason, "frontmatter lang");
    assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(managedZhBytes).digest("hex"));
    assert.equal(await readFile(join(directory, managedZhMetadata), "utf8"), managedZhBytes);

    const quotedMetadata = "\uFEFF---\nlang: 'zh-Hant'\n---\n繁體內容。\n";
    await writeFile(join(directory, managedZhMetadata), quotedMetadata, "utf8");
    for (const spelling of [managedZhMetadata, join(directory, managedZhMetadata), `docs/i18n/zh-Hant/../zh-Hant/lang-guide.md`]) {
      ({ result, report } = await (async () => {
        const selected = await run(["--files", spelling], directory);
        return { result: selected, report: JSON.parse(await readFile(reportFrom(selected.stderr), "utf8")) };
      })());
      assert.ok([0, 1].includes(result.code), result.stderr);
      assert.equal(report.selectedFiles[0].locale, "zh-TW");
      assert.equal(report.selectedFiles[0].reason, "frontmatter lang");
      assert.equal(report.selectedFiles[0].path, spelling, "report preserves caller path spelling");
      assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(quotedMetadata).digest("hex"));
      assert.equal(await readFile(join(directory, managedZhMetadata), "utf8"), quotedMetadata);
    }

    const traversalZh = "docs/i18n/zh-Hant/../../guide.md";
    await writeFile(join(directory, "docs/guide.md"), "---\nlang: zh-Hant\n---\n繁體內容。\n", "utf8");
    const traversal = await run(["--files", traversalZh], directory);
    assert.equal(traversal.code, 2, traversal.stderr);
    assert.match(traversal.stderr, /unsupported frontmatter lang/);

    const outsideZh = "docs/guide.md";
    const original = "---\nlang: zh-Hant\n---\n繁體內容。\n";
    await writeFile(join(directory, outsideZh), original, "utf8");
    ({ result, report } = await check(outsideZh));
    const outsideZhResult = result;
    ({ result, report } = await (async () => {
      const override = await run(["--locale", "en-US", "--files", outsideZh], directory);
      return { result: override, report: JSON.parse(await readFile(reportFrom(override.stderr), "utf8")) };
    })());
    assert.ok([0, 1].includes(result.code), result.stderr);
    assert.equal(report.selectedFiles[0].locale, "en-US");
    assert.equal(report.selectedFiles[0].reason, "explicit override");
    assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(original).digest("hex"));
    assert.equal(await readFile(join(directory, outsideZh), "utf8"), original);

    const aliasFailures = [];
    for (const [alias, expected] of [["en", "en-US"], ["ja", "ja-JP"]]) {
      const file = `${alias}.md`;
      const bytes = "---\nlang: not-a-locale\n---\ntext\n";
      await writeFile(join(directory, file), bytes, "utf8");
      const override = await run(["--locale", alias, "--files", file], directory);
      if (![0, 1].includes(override.code)) {
        aliasFailures.push(`${alias}: exit ${override.code}, ${override.stderr.trim()}`);
        continue;
      }
      const selected = JSON.parse(await readFile(reportFrom(override.stderr), "utf8")).selectedFiles[0];
      if (selected.locale !== expected || selected.reason !== "explicit override" || selected.sha256 !== createHash("sha256").update(bytes).digest("hex")) aliasFailures.push(`${alias}: ${JSON.stringify(selected)}`);
      if (await readFile(join(directory, file), "utf8") !== bytes) aliasFailures.push(`${alias}: input bytes changed`);
    }

    for (const [name, metadata] of [
      ["duplicate", "lang: en\nlang: ja"],
      ["empty", "lang:"],
      ["compound", "lang: en ja"],
      ["unsupported", "lang: fr"],
    ]) {
      const file = `${name}.md`;
      await writeFile(join(directory, file), `---\n${metadata}\n---\ntext\n`, "utf8");
      const invalid = await run(["--files", file], directory);
      assert.equal(invalid.code, 2, name);
      assert.match(invalid.stderr, /frontmatter lang|unknown path/, name);
    }

    const unmanaged = "docs/i18n/fr/guide.md";
    await mkdir(dirname(join(directory, unmanaged)), { recursive: true });
    await writeFile(join(directory, unmanaged), "Texte français.\n", "utf8");
    const unknown = await run(["--files", unmanaged], directory);
    const unknownPathFailsClosed = unknown.code === 2 && /unknown path/.test(unknown.stderr);
    const outsideZhFailsClosed = outsideZhResult.code === 2 && /unsupported frontmatter lang/.test(outsideZhResult.stderr);
    assert.equal(unknownPathFailsClosed, true, `docs/i18n/fr unexpectedly resolved: exit ${unknown.code}, ${unknown.stderr.trim()}`);
    assert.equal(outsideZhFailsClosed, true, `outside zh-Hant unexpectedly resolved: exit ${outsideZhResult.code}, ${outsideZhResult.stderr.trim()}`);
    assert.deepEqual(aliasFailures, [], aliasFailures.join("\n"));
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("managed locale identity survives path normalization and rejects prefix siblings", async () => {
  const parent = await mkdtemp(join(tmpdir(), "gal-locale-path-identity-"));
  const directory = join(parent, "documents");
  const sibling = join(parent, "documents-outside");
  try {
    for (const auth of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await mkdir(dirname(join(directory, auth)), { recursive: true });
      await copyFile(join(root, auth), join(directory, auth));
    }
    const managed = join(directory, "docs", "i18n", "zh-Hant", "guide.md");
    const unmanaged = join(sibling, "docs", "i18n", "zh-Hant", "guide.md");
    await mkdir(dirname(managed), { recursive: true });
    await mkdir(dirname(unmanaged), { recursive: true });
    await writeFile(managed, "繁體內容。\n", "utf8");
    await writeFile(unmanaged, "繁體內容。\n", "utf8");

    const normalizedRelative = await run(["--files", "docs/i18n/zh-Hant/./guide.md"], directory);
    assert.ok([0, 1].includes(normalizedRelative.code), normalizedRelative.stderr);
    const relativeReport = JSON.parse(await readFile(reportFrom(normalizedRelative.stderr), "utf8"));
    assert.equal(relativeReport.selectedFiles[0].locale, "zh-TW");
    assert.equal(relativeReport.selectedFiles[0].reason, "managed path");

    const absolute = await run(["--files", managed], directory);
    assert.ok([0, 1].includes(absolute.code), absolute.stderr);
    const absoluteReport = JSON.parse(await readFile(reportFrom(absolute.stderr), "utf8"));
    assert.equal(absoluteReport.selectedFiles[0].locale, "zh-TW");
    assert.equal(absoluteReport.selectedFiles[0].reason, "managed path");

    const outside = await run(["--files", unmanaged], directory);
    assert.equal(outside.code, 2, outside.stderr);
    assert.match(outside.stderr, /unknown path/);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
});

test("runs an explicit file with the selected profile and writes a complete report", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-check-"));
  const file = join(directory, "sample.md");
  try {
    await writeFile(file, "GAL generated the report.\n", "utf8");
    const result = await run(["--locale", "en-US", "--files", file]);
    assert.equal(result.code, 0, result.stderr);
    const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.equal(report.schemaVersion, 1);
    assert.equal(report.profiles[0], "en-US");
    assert.equal(report.selectedFiles.length, 1);
    assert.equal(report.selectedFiles[0].path.replaceAll("/", "\\"), file);
    assert.equal(report.selectedFiles[0].findings.length, 0);
    assert.equal(report.hardFindings.length, 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("returns hard findings while preserving protected literals", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-hard-"));
  const file = join(directory, "sample.md");
  try {
    await writeFile(file, "The report was generated by GAL.\n\n`The report was generated by GAL.`\n", "utf8");
    const result = await run(["--locale", "en-US", "--files", file]);
    assert.equal(result.code, 0, result.stderr);
    const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.ok(report.findings.some((finding) => finding.ruleId === "write-good"));
    assert.equal(report.hardFindings.length, 0);
    assert.equal(report.findings.some((finding) => finding.message?.includes("literal")), false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("required mode selects tracked files and keeps report data per file", async () => {
  const result = await run(["--required"]);
  assert.ok([0, 1].includes(result.code), result.stderr);
  const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
  assert.ok(report.selectedFiles.some((file) => file.path === "README.md"));
  assert.ok(report.selectedFiles.every((file) => typeof file.sha256 === "string" && file.sha256.length === 64));
  assert.equal(report.selectedFiles.some((file) => file.path.includes("node_modules")), false);
});

test("native CLI loads every trusted locale profile and preserves advisory severity", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-native-"));
  const file = join(directory, "sample.md");
  try {
    await writeFile(file, "The report was generated by GAL.\n", "utf8");
    for (const config of ["en-US.json", "zh-TW.json", "ja-JP.json"]) {
      const result = await runNative(config, file);
      assert.equal(result.code, 0, `${config}: ${result.stderr}`);
      const report = JSON.parse(result.stdout);
      assert.ok(Array.isArray(report));
      assert.ok(report[0] && Array.isArray(report[0].messages));
    }
    const report = JSON.parse((await runNative("en-US.json", file)).stdout);
    assert.ok(report[0].messages.some((finding) => finding.ruleId === "write-good" && finding.severity === 1));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("native CLI keeps terminology errors hard while heuristic advice stays advisory", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-severity-"));
  const file = join(directory, "sample.md");
  try {
    const retiredForm = ["MCP", "provider"].join(" ");
    await writeFile(file, `The ${retiredForm} was generated.\n`, "utf8");
    const result = await runNative("en-US.json", file);
    assert.equal(result.code, 1, result.stderr);
    const report = JSON.parse(result.stdout);
    const messages = report[0].messages;
    assert.ok(messages.some((finding) => finding.ruleId.endsWith("terminology.mjs") && finding.severity === 2));
    assert.ok(messages.some((finding) => finding.ruleId === "write-good" && finding.severity === 1));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("native CLI exposes terminology advice as a warning", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-advice-"));
  const file = join(directory, "sample.md");
  try {
    await writeFile(file, "The coding agent is ready.\n", "utf8");
    const result = await runNative("en-US.json", file);
    assert.equal(result.code, 0, result.stderr);
    const messages = JSON.parse(result.stdout)[0].messages;
    assert.ok(messages.some((finding) => finding.ruleId.endsWith("terminology.mjs") && finding.severity === 1));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("manual suggestions retain sources, ranges, severities, and protected input through checker, native CLI, and MCP", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-suggestions-"));
  const cases = [
    { locale: "en-US", prose: `The ${["MCP", "provider"].join(" ")} was generated in order to help the coding agent.`, replacement: "MCP host", severity: 2 },
    { locale: "zh-TW", prose: "我們推薦使用編碼代理程式。", replacement: "編碼代理程式 (coding agent)", severity: 1 },
    { locale: "ja-JP", prose: `${"あ".repeat(100)}。ないわけではない。提供元を使います。`, replacement: "ベンダー", severity: 1 },
  ];
  try {
    for (const { locale, prose, replacement, severity } of cases) {
      const file = join(directory, `${locale}.md`);
      const source = `😀 ${prose}\r\n\r\n\`${prose}\`\r\n\r\n> ${prose}\r\n`;
      await writeFile(file, source, "utf8");
      const before = await readFile(file);
      const checked = await run(["--locale", locale, "--files", file]);
      assert.equal(checked.code, severity === 2 ? 1 : 0, checked.stderr);
      const report = JSON.parse(await readFile(reportFrom(checked.stderr), "utf8"));
      assert.ok(report.findings.length > 0);
      const term = report.findings.find((finding) => finding.ruleId === "GAL/terminology");
      assert.ok(term, locale);
      assert.equal(term.suggestion.replacement, replacement);
      assert.equal(term.severity, severity);
      if (locale === "en-US") {
        const advice = report.findings.filter((finding) => finding.ruleId === "write-good");
        assert.ok(advice.some((finding) => /Do not invent an actor/.test(finding.suggestion.text)));
        assert.ok(advice.some((finding) => /shorter equivalent/.test(finding.suggestion.text)));
      }
      if (locale === "ja-JP") {
        assert.ok(report.findings.some((finding) => /sentence-length$/.test(finding.ruleId) && /logical boundary/.test(finding.suggestion.text)));
        assert.ok(report.findings.some((finding) => /no-double-negative-ja$/.test(finding.ruleId) && /certainty and scope/.test(finding.suggestion.text)));
      }
      for (const finding of report.findings) {
        assert.ok(finding.suggestion.text.length > 20);
        assert.equal(finding.suggestion.applicability, "manual");
        assert.ok(finding.suggestion.replacement === null || typeof finding.suggestion.replacement === "string");
        assert.equal(finding.policySource, finding.sourceAnchor);
        assert.ok(finding.policySource);
        assert.deepEqual(finding.sourceRange, finding.range);
        assert.ok(finding.range[0] < source.indexOf("\r\n"), JSON.stringify(finding));
        assert.equal(finding.fix, undefined);
      }
      const native = await runNative(`${locale}.json`, file);
      assert.equal(native.code, severity === 2 ? 1 : 0, native.stderr);
      const cliMessages = JSON.parse(native.stdout)[0].messages;
      const mcp = await runMcp(`${locale}.json`, source, `${locale}.md`);
      assert.equal(mcp.isError, false);
      const mcpMessages = JSON.parse(mcp.content.find((item) => item.type === "text").text).messages;
      for (const messages of [cliMessages, mcpMessages]) {
        const terms = messages.filter((finding) => finding.ruleId.endsWith("terminology.mjs"));
        assert.ok(terms.length > 0);
        assert.ok(terms.some((finding) => finding.data.suggestion.replacement === replacement));
        assert.ok(terms.some((finding) => finding.severity === severity));
        for (const finding of terms) {
          assert.equal(finding.data.suggestion.applicability, "manual");
          assert.ok(finding.data.sourceAnchor);
          assert.equal(finding.fix, undefined);
          assert.ok(finding.range[0] < source.indexOf("\r\n"));
        }
      }
      assert.deepEqual(await readFile(file), before);
      assert.equal(report.selectedFiles[0].sha256, createHash("sha256").update(before).digest("hex"));
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("rejects missing selected files and records the skipped reason", async () => {
  const result = await run(["--locale", "en-US", "--files", "missing writing file.md"]);
  assert.equal(result.code, 2);
  const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
  assert.equal(report.completed, false);
  assert.equal(report.skippedFiles[0].path, "missing writing file.md");
  assert.match(report.skippedFiles[0].reason, /does not exist/);
});

test("selects only the two Japanese advisory subrules", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-writing-ja-"));
  const file = join(directory, "sample.md");
  try {
    await writeFile(file, `${"あ".repeat(99)}。\n${"あ".repeat(100)}。\nないわけではない。\n`, "utf8");
    const result = await runNative("ja-JP.json", file);
    assert.equal(result.code, 0, result.stderr);
    const messages = JSON.parse(result.stdout)[0].messages;
    assert.ok(messages.some((finding) => finding.ruleId.endsWith("/sentence-length") && finding.severity === 1));
    assert.ok(messages.some((finding) => finding.ruleId.endsWith("/no-double-negative-ja") && finding.severity === 1));
    assert.equal(messages.some((finding) => finding.ruleId === "max-comma"), false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("official MCP exposes all trusted locale rules and preserves native severities", async () => {
  const cases = [
    ["en-US.json", "The report was generated by GAL.", "write-good", 1],
    ["zh-TW.json", "我們推薦使用編碼代理程式進行開發。", "terminology.mjs", 1],
    ["ja-JP.json", `${"あ".repeat(100)}。`, "sentence-length", 1],
    ["en-US.json", `The ${["MCP", "provider"].join(" ")} is ready.`, "terminology.mjs", 2],
    ["zh-TW.json", "這是一個代碼代理工具。", "terminology.mjs", 2],
    ["ja-JP.json", "提供元を使います。", "terminology.mjs", 1],
  ];
  for (const [config, text, rulePart, severity] of cases) {
    const result = await runMcp(config, text);
    assert.equal(result.isError, false, config);
    const output = result.content.find((item) => item.type === "text")?.text;
    assert.ok(output, config);
    const payload = JSON.parse(output);
    const messages = Array.isArray(payload) ? payload : payload.messages;
    assert.ok(Array.isArray(messages), `${config}: ${output}`);
    assert.ok(messages.some((message) => message.ruleId.includes(rulePart) && message.severity === severity), `${config}: ${output}`);
  }
});

test("required corpus selection includes root docs and excludes undeclared paths", async () => {
  const result = await run(["--required"]);
  assert.ok([0, 1].includes(result.code), result.stderr);
  const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
  const selected = new Set(report.selectedFiles.map((file) => file.path.replaceAll("\\", "/")));
  for (const file of ["README.md", "CONTRIBUTING.md", ".dev/project.md", "docs/glossary.md", "docs/architecture.md", "plugins/gal-core/templates/project.md"]) assert.ok(selected.has(file), file);
  for (const file of [".dev/state.md", "tools/writing/README.md", "docs/structure/structure-map.schema.json"]) assert.equal(selected.has(file), false, file);
});

test("disposable Git corpus exact-set equality with root/nested docs, managed locales, spaces and exclusions", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-git-corpus-"));
  try {
    execFileSync("git", ["init"], { cwd: directory });
    const expectedFiles = [
      "README.md",
      "CONTRIBUTING.md",
      ".dev/project.md",
      "docs/overview.md",
      "docs/glossary.md",
      "docs/nested/guide with spaces.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
      "plugins/gal-core/conventions/writing-quality.md",
      "plugins/gal-core/templates/project with spaces.md",
    ];
    const excludedFiles = [
      "tools/writing/README.md",
      ".dev/state.md",
      "src/main.rs",
      "docs/assets/diagram.png",
      "scratch.txt",
    ];
    for (const file of [...expectedFiles, ...excludedFiles]) {
      await mkdir(dirname(join(directory, file)), { recursive: true });
      await writeFile(join(directory, file), "# Title\nProse content.\n", "utf8");
    }
    for (const authority of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await copyFile(join(root, authority), join(directory, authority));
    }
    execFileSync("git", ["add", "-A"], { cwd: directory });
    const result = await run(["--required"], directory);
    assert.equal(result.code, 0, result.stderr);
    const report = JSON.parse(await readFile(reportFrom(result.stderr), "utf8"));
    assert.equal(report.completed, true);
    assert.deepEqual(report.skippedFiles, []);
    const selected = report.selectedFiles.map((file) => file.path);
    assert.deepEqual(new Set(selected), new Set(expectedFiles));
    for (const excluded of excludedFiles) {
      assert.equal(selected.includes(excluded), false, `excluded file should not be selected: ${excluded}`);
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("actual CLI execution from a complete second cwd with relative inputs", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-second-cwd-"));
  try {
    for (const auth of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await mkdir(dirname(join(directory, auth)), { recursive: true });
      await copyFile(join(root, auth), join(directory, auth));
    }
    const sample = join(directory, "nested", "sample.md");
    await mkdir(dirname(sample), { recursive: true });
    await writeFile(sample, "GAL generated the report.\n", "utf8");
    const result = await new Promise((resolvePromise) => {
      const child = spawn(process.execPath, [checker, "--locale", "en-US", "--files", "nested/sample.md"], { cwd: directory, windowsHide: true });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (chunk) => { stdout += chunk; });
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.once("close", (code) => resolvePromise({ code, stdout, stderr }));
    });
    assert.equal(result.code, 0, result.stderr);
    const reportPath = reportFrom(result.stderr);
    assert.ok(reportPath.startsWith(directory));
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    assert.equal(report.completed, true);
    assert.equal(report.status, "passed");
    assert.equal(report.selectedFiles.length, 1);
    assert.equal(report.selectedFiles[0].path, "nested/sample.md");
    assert.equal(report.hardFindings.length, 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("protected vs ordinary upstream sentinels on native CLI and real official SDK MCP", async () => {
  const text = "The report was generated by GAL.\n\n`The report was generated by GAL.`\n";
  const directory = await mkdtemp(join(tmpdir(), "gal-sentinel-"));
  const file = join(directory, "sentinel.md");
  try {
    await writeFile(file, text, "utf8");
    const cliResult = await runNative("en-US.json", file);
    assert.equal(cliResult.code, 0, cliResult.stderr);
    const cliFindings = JSON.parse(cliResult.stdout)[0].messages;
    const writeGoodCli = cliFindings.filter((m) => m.ruleId === "write-good");
    assert.equal(writeGoodCli.length, 1, "Only ordinary prose generates advisory write-good finding");
    assert.equal(writeGoodCli[0].line, 1);

    const mcpResult = await runMcp("en-US.json", text, "sentinel.md");
    assert.equal(mcpResult.isError, false);
    const mcpText = mcpResult.content.find((item) => item.type === "text")?.text;
    const mcpData = JSON.parse(mcpText);
    const mcpMessages = Array.isArray(mcpData) ? mcpData : mcpData.messages;
    const writeGoodMcp = mcpMessages.filter((m) => m.ruleId === "write-good");
    assert.equal(writeGoodMcp.length, 1, "Only ordinary prose generates advisory finding on MCP route");
    assert.equal(writeGoodMcp[0].line, 1);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("full all-locale clean, hard, and advisory roster with genuine Japanese 100/101 and double-negative cases", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-all-roster-"));
  try {
    const enFile = join(directory, "en.md");
    await writeFile(enFile, "In order to test, we run.\n\nGAL generated the report.\n", "utf8");
    const enRes = await runNative("en-US.json", enFile);
    assert.equal(enRes.code, 0);
    const enMsgs = JSON.parse(enRes.stdout)[0].messages;
    assert.ok(enMsgs.some((m) => m.ruleId === "write-good" && m.severity === 1 && m.message.includes("In order to")));

    const zhFile = join(directory, "zh.md");
    await writeFile(zhFile, "GAL 完成報告。\n\n這是模型上下文協議。\n", "utf8");
    const zhRes = await runNative("zh-TW.json", zhFile);
    assert.equal(zhRes.code, 1);
    const zhMsgs = JSON.parse(zhRes.stdout)[0].messages;
    assert.ok(zhMsgs.some((m) => m.ruleId.endsWith("terminology.mjs") && m.severity === 2 && m.message.includes("模型上下文協議")));

    const jaFile = join(directory, "ja.md");
    const exactly100 = "あ".repeat(99) + "。";
    const exactly101 = "あ".repeat(100) + "。";
    const doubleNegative = "それが事件の発端だったといえなくもない。";
    const affirmative = "それが事件の発端だった。";
    await writeFile(jaFile, `${exactly100}\n${exactly101}\n${doubleNegative}\n${affirmative}\n`, "utf8");
    const jaRes = await runNative("ja-JP.json", jaFile);
    assert.equal(jaRes.code, 0);
    const jaMsgs = JSON.parse(jaRes.stdout)[0].messages;
    assert.equal(jaMsgs.some((m) => m.line === 1 && m.ruleId.includes("sentence-length")), false);
    assert.ok(jaMsgs.some((m) => m.line === 2 && m.ruleId.includes("sentence-length") && m.severity === 1));
    assert.ok(jaMsgs.some((m) => m.line === 3 && m.ruleId.includes("no-double-negative-ja") && m.severity === 1));
    assert.equal(jaMsgs.some((m) => m.line === 4 && m.ruleId.includes("no-double-negative-ja")), false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("missing and stale term data rejection over native CLI and official MCP", async () => {
  const scratch = await mkdtemp(join(tmpdir(), "gal-stale-test-"));
  const overlay = join(scratch, "overlay");
  const ot = join(overlay, "tools", "writing");
  try {
    const assets = [
      "tools/writing/check.mjs",
      "tools/writing/terms.mjs",
      "tools/writing/prose.mjs",
      "tools/writing/rules/terminology.mjs",
      "tools/writing/config/en-US.json",
      "docs/glossary.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
    ];
    for (const f of assets) {
      await mkdir(dirname(join(overlay, f)), { recursive: true });
      await copyFile(join(root, f), join(overlay, f));
    }
    await symlink(join(root, "tools", "writing", "node_modules"), join(ot, "node_modules"), "junction");
    await writeFile(join(overlay, "sample.md"), "GAL generated the report.\n", "utf8");

    // 1. Generate valid initial snapshot
    await generate(overlay);
    const initialExpected = buildTerms(await loadSources(overlay));
    const initialSnapshot = snapshotPath(overlay, initialExpected.sourceHash);
    const initialBytes = await readFile(initialSnapshot, "utf8");
    assert.ok(initialBytes.length > 0);

    // 2. Change sources: now old snapshot exists, but current hash snapshot is absent (stale data)
    await writeFile(join(overlay, "docs", "glossary.md"), (await readFile(join(overlay, "docs", "glossary.md"), "utf8")) + "\n", "utf8");
    const currentExpected = buildTerms(await loadSources(overlay));
    assert.notEqual(initialExpected.sourceHash, currentExpected.sourceHash);

    const cliStale = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"));
    assert.notEqual(cliStale.code, 0, "direct CLI rejects stale terminology data");
    assert.match(cliStale.stderr, /terminology data unavailable|missing derived terminology data/);

    const mcpStale = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), process.env, true);
    assert.equal(mcpStale.isError, true, "direct MCP reports stale terminology data");

    // 3. Corrupt JSON at current hash path
    const currentSnapshot = snapshotPath(overlay, currentExpected.sourceHash);
    await writeFile(currentSnapshot, "not-valid-json {", "utf8");
    const cliCorruptJson = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"));
    assert.notEqual(cliCorruptJson.code, 0, "direct CLI rejects corrupt JSON snapshot");
    assert.match(cliCorruptJson.stderr, /terminology data unavailable|invalid/);

    const mcpCorruptJson = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), process.env, true);
    assert.equal(mcpCorruptJson.isError, true, "direct MCP reports corrupt JSON");

    // 4. Corrupt content at current hash path
    await writeFile(currentSnapshot, JSON.stringify({ schemaVersion: 2, entries: [] }), "utf8");
    const cliCorruptContent = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"));
    assert.notEqual(cliCorruptContent.code, 0, "direct CLI rejects corrupt snapshot content");
    assert.match(cliCorruptContent.stderr, /terminology data unavailable|invalid/);

    const mcpCorruptContent = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), process.env, true);
    assert.equal(mcpCorruptContent.isError, true, "direct MCP reports corrupt content");

    // 5. Missing snapshot at current hash path
    await rm(currentSnapshot, { force: true });
    await rm(initialSnapshot, { force: true });
    const cliMissing = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"));
    assert.notEqual(cliMissing.code, 0, "direct CLI rejects missing snapshot");
    assert.match(cliMissing.stderr, /terminology data unavailable|missing derived terminology data/);

    const mcpMissing = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), process.env, true);
    assert.equal(mcpMissing.isError, true, "direct MCP reports missing terminology data");
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

test("missing configuration triggers infrastructure exit 2", async () => {
  const scratch = await mkdtemp(join(tmpdir(), "gal-missing-cfg-"));
  const overlay = join(scratch, "overlay");
  const ot = join(overlay, "tools", "writing");
  try {
    const assets = [
      "tools/writing/check.mjs",
      "tools/writing/terms.mjs",
      "tools/writing/prose.mjs",
      "tools/writing/rules/terminology.mjs",
      "docs/glossary.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
    ];
    for (const f of assets) {
      await mkdir(dirname(join(overlay, f)), { recursive: true });
      await copyFile(join(root, f), join(overlay, f));
    }
    await symlink(join(root, "tools", "writing", "node_modules"), join(ot, "node_modules"), "junction");
    await writeFile(join(overlay, "sample.md"), "GAL generated the report.\n", "utf8");

    await mkdir(join(ot, "config"), { recursive: true });
    const missingRes = await new Promise((res) => {
      const child = spawn(process.execPath, [join(ot, "check.mjs"), "--locale", "en-US", "--files", "sample.md"], { cwd: overlay, windowsHide: true });
      let stderr = "";
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.once("close", (code) => res({ code, stderr }));
    });
    assert.equal(missingRes.code, 2, "Missing config must exit code 2");
    assert.match(missingRes.stderr, /missing locale configuration: en-US/);
    const reportPath = reportFrom(missingRes.stderr);
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    assert.equal(report.completed, false);
    assert.equal(report.status, "infrastructure-failure");
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

for (const transport of ["cli", "mcp"]) test(`partial completed locale results retained on infrastructure failure (${transport})`, async () => {
  const scratch = await mkdtemp(join(tmpdir(), "gal-partial-fail-"));
  const overlay = join(scratch, "overlay");
  const ot = join(overlay, "tools", "writing");
  try {
    const assets = [
      "tools/writing/check.mjs",
      "tools/writing/terms.mjs",
      "tools/writing/prose.mjs",
      "tools/writing/rules/terminology.mjs",
      "tools/writing/config/en-US.json",
      "docs/glossary.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
    ];
    for (const f of assets) {
      await mkdir(dirname(join(overlay, f)), { recursive: true });
      await copyFile(join(root, f), join(overlay, f));
    }
    await symlink(join(root, "tools", "writing", "node_modules"), join(ot, "node_modules"), "junction");
    await mkdir(dirname(join(overlay, "README.md")), { recursive: true });
    await writeFile(join(overlay, "README.md"), "The report was generated by GAL.\n", "utf8");
    await mkdir(dirname(join(overlay, "docs/i18n/zh-Hant/terminology.zh-Hant.md")), { recursive: true });
    execFileSync("git", ["init"], { cwd: overlay });
    execFileSync("git", ["add", "-A"], { cwd: overlay });
    const result = await new Promise((res) => {
      const child = spawn(process.execPath, [join(ot, "check.mjs"), "--transport", transport, "--required"], { cwd: overlay, windowsHide: true });
      let stderr = "";
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.once("close", (code) => res({ code, stderr }));
    });
    assert.equal(result.code, 2);
    const reportPath = reportFrom(result.stderr);
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    assert.equal(report.completed, false);
    assert.equal(report.status, "infrastructure-failure");
    assert.equal(report.transport, transport);
    if (transport === "mcp") assert.equal(report.servers[0].version, "15.2.3");
    const enFile = report.selectedFiles.find((f) => f.locale === "en-US" && f.path === "README.md");
    assert.ok(enFile, "en-US completed file must be retained in report.selectedFiles");
    assert.ok(enFile.findings.length > 0, "en-US findings must be preserved in partial failure report");
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

test("two concurrent CLI executions produce isolated report directories and valid reports", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-concurrent-"));
  const file1 = join(directory, "file1.md");
  const file2 = join(directory, "file2.md");
  try {
    await writeFile(file1, "GAL generated the report.\n", "utf8");
    await writeFile(file2, "The coding agent is ready.\n", "utf8");
    const [res1, res2] = await Promise.all([
      run(["--locale", "en-US", "--files", file1]),
      run(["--locale", "en-US", "--files", file2]),
    ]);
    assert.equal(res1.code, 0, res1.stderr);
    assert.equal(res2.code, 0, res2.stderr);
    const path1 = reportFrom(res1.stderr);
    const path2 = reportFrom(res2.stderr);
    assert.notEqual(path1, path2, "Concurrent runs must write to distinct report paths");
    const rep1 = JSON.parse(await readFile(path1, "utf8"));
    const rep2 = JSON.parse(await readFile(path2, "utf8"));
    assert.notEqual(rep1.requestId, rep2.requestId, "Concurrent runs must generate unique requestIds");
    assert.equal(rep1.completed, true);
    assert.equal(rep2.completed, true);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("one installation resolves conflicting terminology roots and replaces inherited internal pins", async () => {
  const parent = await mkdtemp(join(tmpdir(), "gal-two-roots-"));
  const roots = [join(parent, "one"), join(parent, "two")];
  try {
    for (const directory of roots) {
      for (const authority of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
        await mkdir(dirname(join(directory, authority)), { recursive: true });
        await copyFile(join(root, authority), join(directory, authority));
      }
      await writeFile(join(directory, "sample.md"), "GAL generated the report.\n", "utf8");
    }
    await writeFile(join(roots[1], "docs/glossary.md"), `${await readFile(join(roots[1], "docs/glossary.md"), "utf8")}\n`, "utf8");
    const inherited = { ...process.env, GAL_WRITING_TERMS_HASH: "malformed-inherited-pin" };
    const results = await Promise.all(roots.map((directory) => run(["--locale", "en-US", "--files", "sample.md"], directory, inherited)));
    for (const result of results) assert.equal(result.code, 0, result.stderr);
    const reports = await Promise.all(results.map(async (result) => JSON.parse(await readFile(reportFrom(result.stderr), "utf8"))));
    assert.notEqual(reports[0].terms.sourceHash, reports[1].terms.sourceHash);
    for (let index = 0; index < roots.length; index += 1) {
      const snapshot = join(roots[index], ".dev", "cache", "writing-terms", `${reports[index].terms.sourceHash}.json`);
      assert.equal(JSON.parse(await readFile(snapshot, "utf8")).sourceHash, reports[index].terms.sourceHash);
    }
  } finally { await rm(parent, { recursive: true, force: true }); }
});

test("one installation resolves genuinely conflicting terminology across two roots on CLI and official MCP", async () => {
  const parent = await mkdtemp(join(tmpdir(), "gal-conflicting-roots-"));
  const rootA = join(parent, "root-a");
  const rootB = join(parent, "root-b");
  try {
    for (const dir of [rootA, rootB]) {
      for (const authority of ["docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
        await mkdir(dirname(join(dir, authority)), { recursive: true });
        await copyFile(join(root, authority), join(dir, authority));
      }
    }

    // In rootA, make "alpha-sentinel-bad" forbidden for mcp-host
    const glossaryA = await readFile(join(rootA, "docs/glossary.md"), "utf8");
    const modifiedA = glossaryA.replace(
      `"forbidden":["${["MCP", "provider"].join(" ")}"]`,
      '"forbidden":["alpha-sentinel-bad"]'
    );
    assert.notEqual(glossaryA, modifiedA);
    await writeFile(join(rootA, "docs/glossary.md"), modifiedA, "utf8");

    // In rootB, make "beta-sentinel-bad" forbidden for mcp-host
    const glossaryB = await readFile(join(rootB, "docs/glossary.md"), "utf8");
    const modifiedB = glossaryB.replace(
      `"forbidden":["${["MCP", "provider"].join(" ")}"]`,
      '"forbidden":["beta-sentinel-bad"]'
    );
    assert.notEqual(glossaryB, modifiedB);
    await writeFile(join(rootB, "docs/glossary.md"), modifiedB, "utf8");

    const sampleAlpha = "Here is alpha-sentinel-bad in text.\n";
    const sampleBeta = "Here is beta-sentinel-bad in text.\n";
    await writeFile(join(rootA, "sample-alpha.md"), sampleAlpha, "utf8");
    await writeFile(join(rootA, "sample-beta.md"), sampleBeta, "utf8");
    await writeFile(join(rootB, "sample-alpha.md"), sampleAlpha, "utf8");
    await writeFile(join(rootB, "sample-beta.md"), sampleBeta, "utf8");

    // CLI checks in rootA: alpha is forbidden (exit 1), beta is clean (exit 0)
    const cliAAlpha = await run(["--locale", "en-US", "--files", "sample-alpha.md"], rootA);
    assert.equal(cliAAlpha.code, 1, "rootA CLI must flag alpha-sentinel-bad as hard finding");
    assert.match(cliAAlpha.stderr, /alpha-sentinel-bad/);

    const cliABeta = await run(["--locale", "en-US", "--files", "sample-beta.md"], rootA);
    assert.equal(cliABeta.code, 0, "rootA CLI must not flag beta-sentinel-bad");

    // CLI checks in rootB: beta is forbidden (exit 1), alpha is clean (exit 0)
    const cliBBeta = await run(["--locale", "en-US", "--files", "sample-beta.md"], rootB);
    assert.equal(cliBBeta.code, 1, "rootB CLI must flag beta-sentinel-bad as hard finding");
    assert.match(cliBBeta.stderr, /beta-sentinel-bad/);

    const cliBAlpha = await run(["--locale", "en-US", "--files", "sample-alpha.md"], rootB);
    assert.equal(cliBAlpha.code, 0, "rootB CLI must not flag alpha-sentinel-bad");

    // Both roots now have snapshots. The same direct native CLI installation
    // must select the current root rather than the most recently generated one.
    for (const [cwd, file, forbidden] of [
      [rootA, "sample-alpha.md", "alpha-sentinel-bad"],
      [rootB, "sample-beta.md", "beta-sentinel-bad"],
    ]) {
      const native = await runNative("en-US.json", file, cwd);
      assert.equal(native.code, 1, native.stderr);
      assert.ok(JSON.parse(native.stdout)[0].messages.some((finding) => finding.severity === 2 && finding.message.includes(forbidden)));
    }
    for (const [cwd, file] of [[rootA, "sample-beta.md"], [rootB, "sample-alpha.md"]]) {
      const native = await runNative("en-US.json", file, cwd);
      assert.equal(native.code, 0, native.stderr);
      assert.equal(JSON.parse(native.stdout)[0].messages.filter((finding) => finding.severity === 2).length, 0);
    }

    // Official MCP checks in rootA and rootB (snapshots already generated by CLI run)
    const mcpAAlpha = await runMcp("en-US.json", sampleAlpha, "sample-alpha.md", rootA);
    assert.equal(mcpAAlpha.isError, false);
    const mcpAAlphaMsgs = JSON.parse(mcpAAlpha.content[0].text).messages;
    assert.ok(mcpAAlphaMsgs.some((m) => m.message.includes("alpha-sentinel-bad")));

    const mcpABeta = await runMcp("en-US.json", sampleBeta, "sample-beta.md", rootA);
    assert.equal(mcpABeta.isError, false);
    const mcpABetaMsgs = JSON.parse(mcpABeta.content[0].text).messages;
    assert.equal(mcpABetaMsgs.filter((m) => m.severity === 2).length, 0);

    const mcpBBeta = await runMcp("en-US.json", sampleBeta, "sample-beta.md", rootB);
    assert.equal(mcpBBeta.isError, false);
    const mcpBBetaMsgs = JSON.parse(mcpBBeta.content[0].text).messages;
    assert.ok(mcpBBetaMsgs.some((m) => m.message.includes("beta-sentinel-bad")));

    const mcpBAlpha = await runMcp("en-US.json", sampleAlpha, "sample-alpha.md", rootB);
    assert.equal(mcpBAlpha.isError, false);
    const mcpBAlphaMsgs = JSON.parse(mcpBAlpha.content[0].text).messages;
    assert.equal(mcpBAlphaMsgs.filter((m) => m.severity === 2).length, 0);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
});

test("parent pin drift and malformed pins rejected on direct CLI/MCP while wrapper replaces inherited pins", async () => {
  const scratch = await mkdtemp(join(tmpdir(), "gal-pin-drift-"));
  const overlay = join(scratch, "overlay");
  const ot = join(overlay, "tools", "writing");
  try {
    const assets = [
      "tools/writing/check.mjs",
      "tools/writing/terms.mjs",
      "tools/writing/prose.mjs",
      "tools/writing/rules/terminology.mjs",
      "tools/writing/config/en-US.json",
      "docs/glossary.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
    ];
    for (const f of assets) {
      await mkdir(dirname(join(overlay, f)), { recursive: true });
      await copyFile(join(root, f), join(overlay, f));
    }
    await symlink(join(root, "tools", "writing", "node_modules"), join(ot, "node_modules"), "junction");
    await writeFile(join(overlay, "sample.md"), "GAL generated the report.\n", "utf8");

    // Generate snapshot A for initial sources
    await generate(overlay);
    const expectedA = buildTerms(await loadSources(overlay));
    const hashA = expectedA.sourceHash;

    // Change sources in overlay to create sources B, and generate snapshot B
    await writeFile(join(overlay, "docs", "glossary.md"), (await readFile(join(overlay, "docs", "glossary.md"), "utf8")) + "\n", "utf8");
    await generate(overlay);
    const expectedB = buildTerms(await loadSources(overlay));
    const hashB = expectedB.sourceHash;
    assert.notEqual(hashA, hashB);

    // Both snapshots exist on disk
    const snapA = snapshotPath(overlay, hashA);
    const snapB = snapshotPath(overlay, hashB);
    assert.ok(await readFile(snapA, "utf8"));
    assert.ok(await readFile(snapB, "utf8"));

    // 1. Direct CLI with parent pin A vs current sources B: fails operationally even though both snapshots exist
    const cliPinA = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"), {
      ...process.env,
      GAL_WRITING_TERMS_HASH: hashA,
    });
    assert.notEqual(cliPinA.code, 0, "direct CLI with drifted parent pin must fail");
    assert.match(cliPinA.stderr, /does not match parent pin|terminology data unavailable/);

    // 2. Direct MCP with parent pin A vs current sources B: fails operationally
    const mcpPinA = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), {
      ...process.env,
      GAL_WRITING_TERMS_HASH: hashA,
    }, true);
    assert.equal(mcpPinA.isError, true, "direct MCP with drifted parent pin must report error");

    // 3. Direct CLI with malformed pin: fails operationally
    const cliMalformed = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"), {
      ...process.env,
      GAL_WRITING_TERMS_HASH: "malformed-pin-000",
    });
    assert.notEqual(cliMalformed.code, 0, "direct CLI with malformed pin must fail");
    assert.match(cliMalformed.stderr, /does not match parent pin|terminology data unavailable/);

    // 4. Direct MCP with malformed pin: fails operationally
    const mcpMalformed = await runMcp(join(ot, "config", "en-US.json"), "GAL generated the report.", "sample.md", overlay, join(ot, "node_modules"), {
      ...process.env,
      GAL_WRITING_TERMS_HASH: "malformed-pin-000",
    }, true);
    assert.equal(mcpMalformed.isError, true, "direct MCP with malformed pin must report error");

    // 5. Direct CLI with matching pin B: succeeds
    const cliPinB = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"), {
      ...process.env,
      GAL_WRITING_TERMS_HASH: hashB,
    });
    assert.equal(cliPinB.code, 0, `direct CLI with matching pin must succeed: ${cliPinB.stderr}`);

    // 6. Direct CLI unpinned: succeeds
    const unpinnedEnv = { ...process.env };
    delete unpinnedEnv.GAL_WRITING_TERMS_HASH;
    const cliUnpinned = await runNative(join(ot, "config", "en-US.json"), "sample.md", overlay, join(ot, "node_modules"), unpinnedEnv);
    assert.equal(cliUnpinned.code, 0, `direct unpinned CLI must succeed: ${cliUnpinned.stderr}`);

    // 7. Wrapper check.mjs with inherited pin A: wrapper replaces inherited pin with hashB and succeeds
    const wrapperInheritedA = await run(["--locale", "en-US", "--files", "sample.md"], overlay, {
      ...process.env,
      GAL_WRITING_TERMS_HASH: hashA,
    });
    assert.equal(wrapperInheritedA.code, 0, `wrapper must replace inherited pin A: ${wrapperInheritedA.stderr}`);

    // 8. Wrapper check.mjs with inherited malformed pin: wrapper replaces inherited pin and succeeds
    const wrapperMalformed = await run(["--locale", "en-US", "--files", "sample.md"], overlay, {
      ...process.env,
      GAL_WRITING_TERMS_HASH: "malformed-inherited-pin",
    });
    assert.equal(wrapperMalformed.code, 0, `wrapper must replace malformed inherited pin: ${wrapperMalformed.stderr}`);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

test("console findings capped at 50 while full findings are retained in report and input is unmutated", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-cap-"));
  const file = join(directory, "many-findings.md");
  try {
    const retiredTerm = ["MCP", "provider"].join(" ");
    const content = `${retiredTerm}.\n\n`.repeat(55);
    await writeFile(file, content, "utf8");
    const result = await run(["--locale", "en-US", "--files", file]);
    assert.equal(result.code, 1, "Hard findings must exit code 1");
    const reportPath = reportFrom(result.stderr);
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    assert.equal(report.findings.length, 55, "Report must retain all 55 findings");
    assert.equal(report.hardFindings.length, 55, "Report must retain all 55 hard findings");
    const lines = result.stderr.split(/\r?\n/).filter((l) => l.includes("GAL/terminology"));
    assert.equal(lines.length, 50, "Console output must be capped at 50 findings");
    assert.match(result.stderr, /\.\.\. 5 additional findings are in/);
    const afterContent = await readFile(file, "utf8");
    assert.equal(afterContent, content, "Input file content must remain unmutated");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

for (const transport of ["cli", "mcp"]) test(`watchdog enforces 60-second deadline, terminates owned child, and leaves unrelated sentinel alive (${transport})`, { timeout: 120_000 }, async () => {
  const scratch = await mkdtemp(join(tmpdir(), "gal-watchdog-accept-"));
  const overlay = join(scratch, "overlay");
  const ot = join(overlay, "tools", "writing");
  try {
    const assets = [
      "tools/writing/check.mjs",
      "tools/writing/terms.mjs",
      "tools/writing/prose.mjs",
      "tools/writing/rules/terminology.mjs",
      "tools/writing/config/en-US.json",
      "docs/glossary.md",
      "docs/i18n/zh-Hant/terminology.zh-Hant.md",
      "docs/i18n/ja/terminology.ja.md",
    ];
    for (const f of assets) {
      await mkdir(dirname(join(overlay, f)), { recursive: true });
      await copyFile(join(root, f), join(overlay, f));
    }
    await symlink(join(root, "tools", "writing", "node_modules"), join(ot, "node_modules"), "junction");
    const hangRule = join(ot, "rules", "hang.mjs");
    await writeFile(hangRule, 'import {writeFileSync} from "node:fs"; export default function(context) { return { [context.Syntax.Document]() { writeFileSync("owned-child.pid", String(process.pid)); Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100000); } }; }', "utf8");
    const enConfig = JSON.parse(await readFile(join(ot, "config", "en-US.json"), "utf8"));
    enConfig.rules["../rules/hang.mjs"] = true;
    await writeFile(join(ot, "config", "en-US.json"), JSON.stringify(enConfig, null, 2), "utf8");
    await writeFile(join(overlay, "sample.md"), "GAL generated the report.\n", "utf8");

    const sentinel = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { windowsHide: true });
    let sentinelAliveAtEnd = false;
    try {
      const child = spawn(process.execPath, [join(ot, "check.mjs"), "--transport", transport, "--locale", "en-US", "--files", "sample.md"], { cwd: overlay, windowsHide: true });
      let stderr = "";
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      const code = await new Promise((res) => child.once("close", res));
      assert.equal(code, 2, "Timed out child execution must exit code 2");
      sentinelAliveAtEnd = sentinel.exitCode === null;
      assert.equal(sentinelAliveAtEnd, true, "Unrelated sentinel process must survive checker watchdog timeout");
      const reportPath = reportFrom(stderr);
      const report = JSON.parse(await readFile(reportPath, "utf8"));
      assert.equal(report.transport, transport);
      assert.equal(report.error?.timedOut, true, "Failure report must record error.timedOut: true");
      if (transport === "mcp") assert.equal(report.servers[0].version, "15.2.3");
      const ownedPid = Number(await readFile(join(overlay, "owned-child.pid"), "utf8"));
      assert.ok(Number.isInteger(ownedPid) && ownedPid > 0);
      assert.throws(() => process.kill(ownedPid, 0), { code: "ESRCH" }, "Owned textlint child must have exited");
      assert.doesNotThrow(() => process.kill(sentinel.pid, 0), "Unrelated sentinel must still exist");
    } finally {
      sentinel.kill();
    }
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

test("all README commands execute against temporary samples including stdin and MCP", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-readme-cmds-"));
  try {
    const sample = join(directory, "sample.md");
    await writeFile(sample, "GAL generated the report.\n", "utf8");

    const termsRes = await new Promise((res) => {
      const child = spawn(process.execPath, [join(root, "tools", "writing", "terms.mjs"), "--check"], { cwd: root, windowsHide: true });
      child.once("close", (code) => res(code));
    });
    assert.equal(termsRes, 0, "terms.mjs --check must exit 0");

    const enRes = await run(["--locale", "en-US", "--files", sample]);
    assert.equal(enRes.code, 0, enRes.stderr);

    const zhSample = join(directory, "sample-zh.md");
    await writeFile(zhSample, "GAL 完成報告。\n", "utf8");
    const zhRes = await run(["--locale", "zh-TW", "--files", zhSample]);
    assert.equal(zhRes.code, 0, zhRes.stderr);

    const jaSample = join(directory, "sample-ja.md");
    await writeFile(jaSample, "GAL は完了しました。\n", "utf8");
    const jaRes = await run(["--locale", "ja-JP", "--files", jaSample]);
    assert.equal(jaRes.code, 0, jaRes.stderr);

    const stdinRes = await new Promise((res) => {
      const child = spawn(process.execPath, [
        join(root, "tools", "writing", "node_modules", "textlint", "bin", "textlint.js"),
        "--config", join(root, "tools", "writing", "config", "en-US.json"),
        "--rules-base-directory", join(root, "tools", "writing", "node_modules"),
        "--stdin",
        "--stdin-filename", "draft.md",
        "--format", "json",
      ], { cwd: root, windowsHide: true });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (chunk) => { stdout += chunk; });
      child.stderr.on("data", (chunk) => { stderr += chunk; });
      child.stdin.end("GAL generated the report.\n");
      child.once("close", (code) => res({ code, stdout, stderr }));
    });
    assert.equal(stdinRes.code, 0, stdinRes.stderr);
    const stdinOutput = JSON.parse(stdinRes.stdout);
    assert.ok(Array.isArray(stdinOutput));

    const mcpRes = await runMcp("en-US.json", "GAL generated the report.", "draft.md");
    assert.equal(mcpRes.isError, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("every locale keeps approved prose clean and forbidden prose hard on native CLI and MCP", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-locale-matrix-"));
  const cases = [
    ["en-US", "GAL generated the report.", `The ${["MCP", "provider"].join(" ")} is ready.`],
    ["zh-TW", "GAL 完成報告。", "這是模型上下文協議。"],
    ["ja-JP", "GAL は完了しました。", "これは対話ボットです。"],
  ];
  try {
    for (const [locale, clean, forbidden] of cases) {
      const file = join(directory, `${locale}.md`);
      for (const [text, hard] of [[clean, false], [forbidden, true]]) {
        await writeFile(file, text, "utf8");
        const native = await runNative(`${locale}.json`, file);
        assert.equal(native.code, hard ? 1 : 0, native.stderr);
        const mcp = await runMcp(`${locale}.json`, text);
        assert.equal(mcp.isError, false, "A terminology finding is a successful tool result");
        const value = JSON.parse(mcp.content.find((item) => item.type === "text").text);
        const messages = Array.isArray(value) ? value : value.messages;
        for (const findings of [JSON.parse(native.stdout)[0].messages, messages]) {
          assert.equal(findings.some((finding) => finding.severity === 2), hard, `${locale}: ${text}`);
          if (!hard) assert.deepEqual(findings, [], `${locale}: approved prose must be clean`);
        }
        assert.equal(await readFile(file, "utf8"), text);
      }
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("malformed native output fails closed through the real checker child boundary", async () => {
  const directory = await mkdtemp(join(tmpdir(), "gal-malformed-output-"));
  const writing = join(directory, "tools", "writing");
  try {
    for (const file of ["tools/writing/check.mjs", "tools/writing/terms.mjs", "docs/glossary.md", "docs/i18n/zh-Hant/terminology.zh-Hant.md", "docs/i18n/ja/terminology.ja.md"]) {
      await mkdir(dirname(join(directory, file)), { recursive: true });
      await copyFile(join(root, file), join(directory, file));
    }
    await mkdir(join(writing, "config"), { recursive: true });
    await writeFile(join(writing, "config", "en-US.json"), "{}", "utf8");
    const fakeCli = join(writing, "node_modules", "textlint", "bin", "textlint.js");
    await mkdir(dirname(fakeCli), { recursive: true });
    await writeFile(join(directory, "sample.md"), "GAL completed the report.", "utf8");
    const message = {ruleId: "write-good", message: "test", severity: 1, range: [0, 3]};
    const result = (finding) => [{filePath: join(directory, "sample.md"), messages: [finding]}];
    const runOverlay = () => new Promise((resolvePromise, reject) => {
        const child = spawn(process.execPath, [join(writing, "check.mjs"), "--locale", "en-US", "--files", "sample.md"], {cwd: directory, windowsHide: true});
        let stderr = "";
        child.stderr.on("data", (chunk) => { stderr += chunk; });
        child.once("error", reject);
        child.once("close", (code) => resolvePromise({code, stderr}));
    });
    const invalidSuggestion = { text: "Apply this", replacement: "value", applicability: "automatic" };
    for (const output of ["invalid JSON", "[]", JSON.stringify(result({...message, severity: 3})), JSON.stringify(result({...message, range: [-1, 2]})), JSON.stringify(result({...message, data: { suggestion: invalidSuggestion }})), JSON.stringify([{filePath: "unexpected.md", messages: []}])]) {
      await writeFile(fakeCli, `process.stdout.write(${JSON.stringify(output)});`, "utf8");
      const res = await runOverlay();
      assert.equal(res.code, 2, res.stderr);
      const report = JSON.parse(await readFile(reportFrom(res.stderr), "utf8"));
      assert.equal(report.completed, false);
      assert.equal(report.status, "infrastructure-failure");
      assert.match(report.error.message, /invalid textlint JSON|textlint JSON must|malformed|unknown/);
    }
    const fix = { range: [0, 3], text: "Changed" };
    await writeFile(fakeCli, `process.stdout.write(${JSON.stringify(JSON.stringify(result({ ...message, fix })))});`, "utf8");
    const preserved = await runOverlay();
    assert.equal(preserved.code, 0, preserved.stderr);
    const report = JSON.parse(await readFile(reportFrom(preserved.stderr), "utf8"));
    assert.deepEqual(report.findings[0].fix, fix);
    assert.equal(report.findings[0].suggestion.applicability, "manual");
    assert.equal(report.findings[0].suggestion.replacement, null);
    assert.equal(await readFile(join(directory, "sample.md"), "utf8"), "GAL completed the report.");
    await writeFile(fakeCli, `require("node:fs").writeFileSync("sample.md", "Changed during check"); process.stdout.write(${JSON.stringify(JSON.stringify(result(message)))});`, "utf8");
    const changed = await runOverlay();
    assert.equal(changed.code, 2, changed.stderr);
    const changedReport = JSON.parse(await readFile(reportFrom(changed.stderr), "utf8"));
    assert.equal(changedReport.completed, false);
    assert.match(changedReport.error.message, /input changed during writing check/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
