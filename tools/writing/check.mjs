import { createHash, randomUUID } from "node:crypto";
import { access, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { generate, check as checkTerms } from "./terms.mjs";

const TOOL_ROOT = dirname(fileURLToPath(import.meta.url));
const CONFIG_ROOT = join(TOOL_ROOT, "config");
const TEXTLINT = join(TOOL_ROOT, "node_modules", "textlint", "bin", "textlint.js");
const DEADLINE_MS = 60_000;
const CONSOLE_LIMIT = 50;
const LOCALE_ALIASES = new Map([["en", "en-US"], ["en-US", "en-US"], ["zh-TW", "zh-TW"], ["ja", "ja-JP"], ["ja-JP", "ja-JP"]]);
const REQUIRED_ROOT_FILES = new Set(["README.md", "CONTRIBUTING.md", ".dev/project.md"]);
const POLICY_SOURCES = {
  "write-good": "tools/writing/rule-provenance.md#adapted-policy-sources",
  "sentence-length": "tools/writing/rule-provenance.md#enabled-rule-roster",
  "no-double-negative-ja": "tools/writing/rule-provenance.md#enabled-rule-roster",
  "ja-technical-writing/sentence-length": "tools/writing/rule-provenance.md#enabled-rule-roster",
  "ja-technical-writing/no-double-negative-ja": "tools/writing/rule-provenance.md#enabled-rule-roster",
};

const hash = (value) => createHash("sha256").update(value).digest("hex");
const infrastructureError = (message, details = {}) => Object.assign(new Error(message), { infrastructure: true, ...details });
const fail = (message, details) => { throw infrastructureError(message, details); };

function parseArgs(argv) {
  const args = { required: false, locale: null, localeProvided: false, transport: "cli", files: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--required") args.required = true;
    else if (value === "--transport") {
      args.transport = argv[++index];
      if (!["cli", "mcp"].includes(args.transport)) fail("--transport must be cli or mcp");
    }
    else if (value === "--locale") {
      args.localeProvided = true;
      args.locale = argv[++index];
      if (args.locale === undefined || args.locale.startsWith("--")) fail("--locale requires a supported locale value");
    }
    else if (value === "--files") { args.files.push(...argv.slice(index + 1)); break; }
    else fail(`unknown argument: ${value}`);
  }
  if (args.required && (args.localeProvided || args.files.length)) fail("--required cannot be combined with --locale or --files");
  if (!args.required && !args.files.length) fail("use --required or --files <paths...>");
  if (args.localeProvided && !LOCALE_ALIASES.has(args.locale)) fail(`unsupported locale: ${args.locale ?? ""}`);
  if (args.localeProvided) args.locale = LOCALE_ALIASES.get(args.locale);
  return args;
}

function isRequiredPath(file) {
  const normalized = file.replaceAll("\\", "/");
  return REQUIRED_ROOT_FILES.has(normalized) ||
    (normalized.startsWith("docs/") && normalized.endsWith(".md")) ||
    (normalized.startsWith("plugins/gal-core/") && normalized.endsWith(".md"));
}

function gitTrackedFiles(root) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn("git", ["ls-files", "-z"], { cwd: root, windowsHide: true });
    const chunks = [];
    let stderr = "";
    child.stdout.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("error", (error) => reject(infrastructureError(`git ls-files failed: ${error.message}`)));
    child.once("close", (code) => {
      if (code !== 0) reject(infrastructureError(stderr.trim() || `git ls-files exited ${code}`));
      else resolvePromise(Buffer.concat(chunks).toString("utf8").split("\0").filter(isRequiredPath));
    });
  });
}

function documentIdentity(root, file) {
  return relative(resolve(root), resolve(root, file)).replaceAll("\\", "/");
}

function managedLocale(identity) {
  const normalized = identity.replaceAll("\\", "/");
  if (normalized.startsWith("docs/i18n/zh-Hant/")) return { locale: "zh-TW", reason: "managed path" };
  if (normalized.startsWith("docs/i18n/ja/")) return { locale: "ja-JP", reason: "managed path" };
  if (normalized === "README.md" || normalized === "CONTRIBUTING.md" || normalized === ".dev/project.md" ||
      (normalized.startsWith("docs/") && normalized.endsWith(".md") && !normalized.startsWith("docs/i18n/")) ||
      (normalized.startsWith("plugins/gal-core/") && normalized.endsWith(".md"))) return { locale: "en-US", reason: "managed path" };
  return null;
}

const LANGUAGES = new Map([["en", "en-US"], ["en-US", "en-US"], ["ja", "ja-JP"], ["ja-JP", "ja-JP"], ["zh-TW", "zh-TW"]]);

function metadataLocale(text, identity) {
  const source = text.replace(/^\uFEFF/, "");
  const lines = source.split(/\r?\n/);
  if (lines[0] !== "---") return null;
  const end = lines.indexOf("---", 1);
  if (end < 0) fail(`unparseable frontmatter lang in ${identity}`);
  const declarations = lines.slice(1, end).filter((line) => /^lang(?:\s|:|$)/.test(line));
  if (!declarations.length) return null;
  if (declarations.length !== 1) fail(`duplicate frontmatter lang in ${identity}`);
  const match = declarations[0].match(/^lang:\s*(?:(['"])(.*?)\1|([^\s#]+))\s*(?:#.*)?$/);
  if (!match) fail(`unparseable frontmatter lang in ${identity}`);
  const value = (match[2] ?? match[3] ?? "").trim();
  if (!value) fail(`empty frontmatter lang in ${identity}`);
  if (/[ ,\t]/.test(value)) fail(`unsupported frontmatter lang: ${value}`);
  if (value === "zh-Hant") {
    if (identity.replaceAll("\\", "/").startsWith("docs/i18n/zh-Hant/")) return "zh-TW";
    fail(`unsupported frontmatter lang: ${value}`);
  }
  if (!LANGUAGES.has(value)) fail(`unsupported frontmatter lang: ${value}`);
  return LANGUAGES.get(value);
}

async function selectLocale(root, file, explicitLocale) {
  if (explicitLocale !== null) return { locale: explicitLocale, reason: "explicit override" };
  const identity = documentIdentity(root, file);
  const text = await readFile(resolve(root, file), "utf8");
  const fromMetadata = metadataLocale(text, identity);
  if (fromMetadata) return { locale: fromMetadata, reason: "frontmatter lang" };
  return managedLocale(identity);
}

async function selectFiles(root, args) {
  const candidates = args.required
    ? (await gitTrackedFiles(root)).map((file) => ({ file: file.replaceAll("\\", "/") }))
    : args.files.map((file) => ({ file }));
  const selected = [];
  const skipped = [];
  for (const item of candidates) {
    try { await access(resolve(root, item.file)); }
    catch { skipped.push({ ...item, path: item.file, reason: "file does not exist" }); continue; }
    const selection = await selectLocale(root, item.file, args.localeProvided ? args.locale : null);
    if (!selection) fail(`unsupported locale for unknown path: ${item.file}`);
    selected.push({ ...item, ...selection, reason: args.localeProvided ? "explicit override" : selection.reason });
  }
  return { selected, skipped };
}

function reportDirectory(root, requestId) { return join(root, ".dev", "pipeline", "writing-quality", requestId); }

function runTextlint(root, config, files, sourceHash) {
  return new Promise((resolvePromise, reject) => {
    const env = { ...process.env, GAL_WRITING_TERMS_HASH: sourceHash };
    const child = spawn(process.execPath, [TEXTLINT, "--config", resolve(config), "--rules-base-directory", resolve(TOOL_ROOT, "node_modules"), "--format", "json", ...files], { cwd: root, windowsHide: true, env });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = (fn, value) => { if (settled) return; settled = true; clearTimeout(timer); fn(value); };
    const timer = setTimeout(() => {
      timedOut = true;
      terminationSucceeded = child.kill();
    }, DEADLINE_MS);
    let timedOut = false;
    let terminationSucceeded = null;
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("close", (code) => {
      if (timedOut) return finish(reject, infrastructureError(`textlint child timed out after ${DEADLINE_MS} ms${terminationSucceeded ? "" : " and termination was not accepted"}`, { timedOut: true, terminationSucceeded }));
      if (code !== 0 && code !== 1) return finish(reject, infrastructureError(stderr.trim() || `textlint exited ${code}`, { code }));
      let parsed;
      try { parsed = JSON.parse(stdout); } catch (error) { return finish(reject, infrastructureError(`invalid textlint JSON: ${error.message}`)); }
      if (!Array.isArray(parsed) || parsed.length !== files.length) return finish(reject, infrastructureError("textlint JSON must be an array with one result per selected file"));
      finish(resolvePromise, { code, results: parsed, stderr });
    });
    child.once("error", (error) => finish(reject, infrastructureError(`textlint child failed: ${error.message}`)));
  });
}

function canonicalPath(root, file) { return resolve(root, file); }

async function runTextlintMcp(root, config, files, sourceHash, textCache, recordServer) {
  const [{ Client }, { StdioClientTransport }] = await Promise.all([
    import("./node_modules/@modelcontextprotocol/sdk/dist/esm/client/index.js"),
    import("./node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js"),
  ]);
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [TEXTLINT, "--mcp", "--config", resolve(config), "--rules-base-directory", resolve(TOOL_ROOT, "node_modules")],
    cwd: root,
    env: { ...process.env, GAL_WRITING_TERMS_HASH: sourceHash },
    stderr: "pipe",
  });
  const client = new Client({ name: "gal-writing-check", version: "1.0.0" });
  let closed;
  const closedPromise = new Promise((resolveClosed) => { closed = resolveClosed; });
  transport.onclose = closed;
  let stderr = "";
  transport.stderr?.on("data", (chunk) => { stderr += chunk; });
  let timer;
  let timedOut = false;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      timedOut = true;
      reject(infrastructureError(`textlint MCP child timed out after ${DEADLINE_MS} ms`, { timedOut: true }));
    }, DEADLINE_MS);
  });
  try {
    const work = async () => {
      await client.connect(transport);
      const server = client.getServerVersion();
      if (server) recordServer(server);
      const available = await client.listTools();
      if (!["lintText", "lintFile"].every((name) => available.tools.some((tool) => tool.name === name))) fail("textlint MCP is missing required lint tools");
      const results = [];
      for (const file of files) {
        const text = textCache.get(file);
        // The official lintText schema rejects an empty string. Use its file
        // tool for empty files without inventing content to satisfy the schema.
        const response = await client.callTool(text.length === 0
          ? { name: "lintFile", arguments: { filePaths: [resolve(root, file)] } }
          : { name: "lintText", arguments: { text, stdinFilename: resolve(root, file) } });
        if (response.isError) fail(`textlint MCP check failed: ${JSON.stringify(response.content)}`);
        const content = response.content.find((item) => item.type === "text")?.text;
        if (!content) fail("textlint MCP returned no result text");
        let result;
        try { result = JSON.parse(content); } catch { fail("textlint MCP returned invalid JSON"); }
        if (result.isError) fail(`textlint MCP check failed: ${result.error ?? "unknown error"}`);
        if (text.length === 0) {
          if (!Array.isArray(result.results) || result.results.length !== 1) fail("textlint MCP returned a malformed empty-file result");
          [result] = result.results;
        }
        results.push(result);
      }
      return { results, stderr };
    };
    return await Promise.race([work(), deadline]);
  } catch (error) {
    if (timedOut) throw infrastructureError(`textlint MCP child timed out after ${DEADLINE_MS} ms`, { timedOut: true });
    throw infrastructureError(`textlint MCP failed: ${error.message}`);
  } finally {
    clearTimeout(timer);
    const hadChild = transport.pid !== null;
    // The SDK closes and, if needed, terminates only its own spawned process.
    await client.close();
    if (hadChild) {
      let closeTimer;
      try {
        await Promise.race([closedPromise, new Promise((_, reject) => {
          closeTimer = setTimeout(() => reject(infrastructureError("textlint MCP child cleanup was not confirmed", { timedOut })), 2000);
        })]);
      } finally { clearTimeout(closeTimer); }
    }
  }
}

function manualSuggestion(finding) {
  const supplied = finding.data?.suggestion;
  if (supplied !== undefined) {
    if (!supplied || typeof supplied.text !== "string" || !supplied.text.trim() || !(supplied.replacement === null || typeof supplied.replacement === "string") || supplied.applicability !== "manual") fail("textlint returned a malformed manual suggestion");
    return { text: supplied.text, replacement: supplied.replacement, applicability: "manual" };
  }
  let text;
  if (finding.ruleId === "write-good") {
    text = /passive voice/i.test(finding.message)
      ? "Consider active voice when the actor is known and relevant. Keep passive voice when changing it would alter the meaning or emphasis. Do not invent an actor."
      : "Review the flagged wording for a shorter equivalent. Preserve necessary conditions, emphasis, and technical meaning.";
  } else if (finding.ruleId.endsWith("sentence-length")) {
    text = "Consider splitting the sentence at a logical boundary. Keep the actor, conditions, negation, and causal relationships explicit.";
  } else if (finding.ruleId.endsWith("no-double-negative-ja")) {
    text = "Consider a direct affirmative expression only when it preserves the original certainty and scope. Keep a necessary double negative.";
  } else {
    text = "Review this occurrence against the cited rule and preserve its intended meaning before editing.";
  }
  return { text, replacement: null, applicability: "manual" };
}

async function verifyInputsUnchanged(root, textCache) {
  for (const [file, text] of textCache) {
    if (await readFile(resolve(root, file), "utf8") !== text) fail(`input changed during writing check: ${file}`);
  }
}

function normalizeResults(root, selected, lint, textCache) {
  const expected = new Map(selected.map((item) => [canonicalPath(root, item.file), item]));
  const seen = new Set();
  const normalized = [];
  for (const item of lint.results) {
    if (!item || typeof item !== "object" || typeof item.filePath !== "string" || !Array.isArray(item.messages)) fail("textlint returned a malformed file result");
    const key = canonicalPath(root, item.filePath);
    const selection = expected.get(key);
    if (!selection || seen.has(key)) fail("textlint returned an unknown or duplicate file result");
    seen.add(key);
    const findings = item.messages.map((finding) => {
      if (!finding || typeof finding !== "object" || typeof finding.ruleId !== "string" || typeof finding.message !== "string" || ![1, 2].includes(finding.severity)) fail("textlint returned a malformed finding");
      const range = finding.range;
      if (!Array.isArray(range) || range.length !== 2 || !range.every(Number.isInteger) || range[0] < 0 || range[1] < range[0]) fail("textlint returned a malformed source range");
      const sourceAnchor = finding.data?.sourceAnchor ?? finding.sourceAnchor ?? finding.message.match(/\[source: ([^\]]+)\]/)?.[1] ?? POLICY_SOURCES[finding.ruleId] ?? null;
      return { ...finding, engine: "textlint", ruleId: finding.ruleId.endsWith("terminology.mjs") ? "GAL/terminology" : finding.ruleId, sourceRange: range, rationale: finding.message, suggestion: manualSuggestion(finding), sourceAnchor, policySource: sourceAnchor };
    });
    normalized.push({ ...selection, path: selection.file, sha256: hash(textCache.get(selection.file)), findings });
  }
  if (seen.size !== expected.size) fail("textlint did not return a result for every selected file");
  return selected.map((item) => normalized.find((result) => result.path === item.file));
}

async function atomicReport(directory, report) {
  await mkdir(directory, { recursive: true });
  const temporary = join(directory, `.report-${process.pid}-${randomUUID()}.tmp`);
  const target = join(directory, "report.json");
  await writeFile(temporary, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  await rename(temporary, target);
  return target;
}

function baseReport(requestId, selected, skipped, completedResults = [], terms = null, textCache = new Map()) {
  const completedPaths = new Set(completedResults.map((item) => item.path));
  const selectedFiles = [
    ...completedResults,
    ...selected
      .filter((item) => !completedPaths.has(item.file))
      .map((item) => ({ ...item, path: item.file, sha256: textCache.has(item.file) ? hash(textCache.get(item.file)) : null, findings: [] })),
  ];
  const findings = completedResults.flatMap((item) => item.findings.map((finding) => ({ ...finding, filePath: item.path, locale: item.locale })));
  return {
    schemaVersion: 1,
    requestId,
    completed: false,
    status: "infrastructure-failure",
    selectedFiles,
    skippedFiles: skipped,
    findings,
    hardFindings: findings.filter((finding) => finding.severity >= 2),
    versions: { node: process.version, ...installedVersions() },
    configHashes: Object.fromEntries([...new Set(selected.map((item) => item.locale))].map((locale) => {
      try { return [locale, hash(readFileSync(join(CONFIG_ROOT, `${locale}.json`), "utf8"))]; } catch { return [locale, null]; }
    })),
    profiles: [...new Set(selected.map((item) => item.locale))],
    terms,
    consoleLimit: CONSOLE_LIMIT,
  };
}

function installedVersions() {
  const get = (name) => {
    try { return JSON.parse(readFileSync(join(TOOL_ROOT, "node_modules", name, "package.json"), "utf8")).version; } catch { return null; }
  };
  return { textlint: get("textlint"), writeGood: get("textlint-rule-write-good"), japanesePreset: get("textlint-rule-preset-ja-technical-writing") };
}

export async function main(argv = process.argv.slice(2), root = process.cwd()) {
  const requestId = randomUUID();
  let selected = [];
  let skipped = [];
  let completedResults = [];
  let terms = null;
  let transport = "cli";
  const servers = [];
  const textCache = new Map();
  try {
    const args = parseArgs(argv);
    transport = args.transport;
    ({ selected, skipped } = await selectFiles(root, args));
    if (skipped.length) fail("one or more selected files do not exist", { selected, skipped });
    if (!selected.length) fail("no existing files selected", { selected, skipped });
    for (const item of selected) textCache.set(item.file, await readFile(resolve(root, item.file), "utf8"));
    await generate(root);
    terms = await checkTerms(root);
    const groups = new Map();
    for (const item of selected) { if (!groups.has(item.locale)) groups.set(item.locale, []); groups.get(item.locale).push(item); }
    const results = [];
    for (const [locale, items] of groups) {
      const config = join(CONFIG_ROOT, `${locale}.json`);
      try { await access(config); } catch { fail(`missing locale configuration: ${locale}`); }
      const files = items.map((item) => item.file);
      const lint = transport === "mcp"
        ? await runTextlintMcp(root, config, files, terms.sourceHash, textCache, (server) => servers.push({ locale, ...server }))
        : await runTextlint(root, config, files, terms.sourceHash);
      completedResults.push(...normalizeResults(root, items, lint, textCache));
    }
    await verifyInputsUnchanged(root, textCache);
    const findings = completedResults.flatMap((item) => item.findings.map((finding) => ({ ...finding, filePath: item.path, locale: item.locale })));
    const hardFindings = findings.filter((finding) => finding.severity >= 2);
    const report = { ...baseReport(requestId, selected, skipped, completedResults, { sourceHash: terms.sourceHash, schemaVersion: terms.schemaVersion }, textCache), transport, servers, completed: true, status: hardFindings.length ? "hard-findings" : "passed", findings, hardFindings, profiles: [...groups.keys()] };
    const path = await atomicReport(reportDirectory(root, requestId), report);
    for (const finding of findings.slice(0, CONSOLE_LIMIT)) console.error(`${finding.filePath}:${finding.line ?? "?"}:${finding.column ?? "?"} ${finding.message}`);
    if (findings.length > CONSOLE_LIMIT) console.error(`... ${findings.length - CONSOLE_LIMIT} additional findings are in ${path}`);
    console.error(`report: ${path}`);
    return hardFindings.length ? 1 : 0;
  } catch (error) {
    const report = { ...baseReport(requestId, selected, skipped, completedResults, terms && { sourceHash: terms.sourceHash, schemaVersion: terms.schemaVersion }, textCache), transport, servers, error: { message: error.message, timedOut: Boolean(error.timedOut) } };
    try { console.error(`report: ${await atomicReport(reportDirectory(root, requestId), report)}`); } catch (reportError) { console.error(`writing check: ${error.message}; report failed: ${reportError.message}`); }
    console.error(`writing check: ${error.message}`);
    return 2;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) main().then((code) => { process.exitCode = code; });
