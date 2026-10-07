import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, link, unlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const SCHEMA_VERSION = 1;
export const AUTHORITY_PATHS = [
  "docs/glossary.md",
  "docs/i18n/zh-Hant/terminology.zh-Hant.md",
  "docs/i18n/ja/terminology.ja.md",
];
export function snapshotPath(root, sourceHash) {
  return join(resolve(root), ".dev", "cache", "writing-terms", `${sourceHash}.json`);
}

const BLOCK = /<!--\s*gal-terms:start\s*-->\s*```json\s*([\s\S]*?)\s*```\s*<!--\s*gal-terms:end\s*-->/g;
const LOCALES = new Set(["en-US", "zh-TW", "ja-JP"]);
const MODES = new Set(["keep-en", "localized", "bilingual-first-use", "transliterated", "contextual"]);
const hash = (text) => createHash("sha256").update(text).digest("hex");

function sourceParts(sources) {
  if (Array.isArray(sources)) return sources.map((source, index) => ({ path: source.path ?? String(index), text: source.text ?? source.content ?? "" }));
  return Object.entries(sources ?? {}).map(([path, text]) => ({ path, text: typeof text === "string" ? text : text.content ?? text.text ?? "" }));
}

function blocks(text, path) {
  const result = [];
  for (const match of text.matchAll(BLOCK)) {
    let value;
    try { value = JSON.parse(match[1]); } catch (error) { throw new Error(`${path}: invalid gal-terms JSON: ${error.message}`); }
    if (value.schemaVersion !== SCHEMA_VERSION || typeof value.locale !== "string" || !Array.isArray(value.entries)) {
      throw new Error(`${path}: gal-terms block must declare schemaVersion=1, locale, and entries`);
    }
    for (const entry of value.entries) {
      for (const field of ["allowed", "forbidden", "scope", "exceptions"]) {
        if (!entry || !Array.isArray(entry[field]) || entry[field].some((item) => typeof item !== "string")) {
          throw new Error(`${path}: ${field} must be an explicit array of strings`);
        }
      }
    }
    result.push({ ...value, sourcePath: path });
  }
  if (!result.length) throw new Error(`${path}: missing explicit gal-terms JSON block`);
  return result;
}

function entryKey(entry) { return `${entry.conceptId}\u0000${entry.locale}`; }
function list(value) { return Array.isArray(value) ? value : value == null ? [] : [value]; }

function checkHumanTables(text, entries, path, block = {}) {
  const cellValue = (value) => {
    const trimmed = value.trim().replace(/`/g, "");
    try {
      return JSON.stringify(JSON.parse(trimmed));
    } catch {
      return trimmed.replace(/\s+/g, " ");
    }
  };
  const lines = text.split(/\r?\n/);
  let heading = "";
  const matched = new Set();
  const englishMatched = new Set();
  for (const entry of entries) {
    if (!entry.sourceAnchor?.includes("#")) continue;
    const [anchorPath, fragment] = entry.sourceAnchor.split("#", 2);
    if (anchorPath !== path || !new RegExp(`(?:id|name)=[\"']${fragment}[\"']`).test(text)) {
      throw new Error(`${path}: missing source anchor for ${entry.conceptId}`);
    }
  }
  for (let index = 0; index + 2 < lines.length; index += 1) {
    const headingMatch = lines[index].match(/^#{2,4}\s+(.+?)\s*$/);
    if (headingMatch) heading = headingMatch[1];
    if (!/^\s*\|/.test(lines[index]) || !/^\s*\|?\s*:?-{3,}/.test(lines[index + 1])) continue;
    const headers = lines[index].split("|").slice(1, -1).map((cell) => cell.trim().replace(/`/g, ""));
    const rows = [];
    for (let row = index + 2; row < lines.length && /^\s*\|/.test(lines[row]); row += 1) rows.push(lines[row].split("|").slice(1, -1).map((cell) => cell.trim().replace(/`/g, "")));
    const termColumn = headers.findIndex((value) => ["conceptId", "英文術語", "英語用語"].includes(value));
    const modeColumn = headers.findIndex((value) => ["mode", "呈現模式", "表示モード"].includes(value));
    const preferredColumn = headers.findIndex((value) => ["preferred", "zh-Hant 固定寫法", "ja 固定表記"].includes(value));
    const forbiddenColumn = headers.findIndex((value) => ["forbidden", "禁止寫法", "避ける表記"].includes(value));
    for (const entry of entries) if (entry.locale === "en-US" && block.sourceTable?.headerOnlyConcepts?.[entry.conceptId] && headers.includes(block.sourceTable.headerOnlyConcepts[entry.conceptId])) englishMatched.add(entry.conceptId);
  for (const entry of entries) {
    if (englishMatched.has(entry.conceptId)) continue;
    if (!entry.sourceRow) {
        if (entry.locale === "en-US" && block.sourceTable) {
          const policy = block.sourceTable;
          const candidates = rows.filter((candidate) => [...policy.termColumns, ...policy.aliasesColumns].some((column) => {
            const value = candidate[headers.indexOf(column)] ?? "";
            return entry.allowed.some((form) => value.replace(/\*\*/g, "").includes(form));
          })).sort((left, right) => {
            const width = (row) => Math.min(...policy.termColumns.filter((column) => headers.includes(column)).map((column) => (row[headers.indexOf(column)] ?? "").length));
            return width(left) - width(right);
          });
          if (candidates.length === 0) continue;
          const row = candidates[0];
          const tied = candidates.filter((candidate) => policy.termColumns.filter((column) => headers.includes(column)).some((column) => (candidate[headers.indexOf(column)] ?? "").length === (row[headers.indexOf(column)] ?? "").length));
          if (tied.length !== 1) throw new Error(`${path}: explicit English source table mapping for ${entry.conceptId} is ambiguous`);
          const lexicalCells = [...policy.aliasesColumns, ...(policy.definitionColumns ?? [])]
            .map((column) => row[headers.indexOf(column)] ?? "").join(" ");
          const termCells = policy.termColumns.map((column) => row[headers.indexOf(column)] ?? "").join(" ");
          const expectedAliases = entry.allowed.filter((form) => form !== entry.preferred);
          for (const form of expectedAliases) if (!termCells.includes(form) && !lexicalCells.includes(form)) throw new Error(`${path}: human table disagrees with ${entry.conceptId}.allowed (${form})`);
          for (const form of entry.forbidden) if (lexicalCells.includes(form)) throw new Error(`${path}: human table disagrees with ${entry.conceptId}.forbidden`);
          englishMatched.add(entry.conceptId);
          continue;
        }
        if (headers.includes("conceptId")) {
          const row = rows.find((candidate) => candidate[termColumn] === entry.conceptId);
          if (row) {
            for (const field of ["locale", "preferred", "allowed", "forbidden", "mode", "scope", "exceptions", "sourceAnchor"]) {
              const column = headers.indexOf(field);
              if (column < 0) continue;
              const expected = cellValue(Array.isArray(entry[field]) ? JSON.stringify(entry[field]) : String(entry[field]));
              const actual = cellValue(row[column]);
              if (actual !== expected && !(Array.isArray(entry[field]) && actual === cellValue(entry[field].join(", ")))) throw new Error(`${path}: human table disagrees with ${entry.conceptId}.${field}`);
            }
          }
        } else if (entry.locale !== "en-US" || termColumn >= 0) throw new Error(`${path}: ${entry.conceptId} is missing explicit sourceRow mapping`);
        continue;
      }
      const normalizeHeading = (value) => value.replace(/註冊/g, "登錄");
      if (normalizeHeading(entry.sourceRow.heading) !== normalizeHeading(heading)) continue;
      const row = rows.find((candidate) => candidate[termColumn] === entry.sourceRow.term);
      if (!row) throw new Error(`${path}: missing mapped human table row for ${entry.conceptId}`);
      matched.add(entry);
      for (const [field, column, expected] of [["mode", modeColumn, entry.sourceRow.mode], ["preferred", preferredColumn, entry.sourceRow.preferred], ["forbidden", forbiddenColumn, entry.sourceRow.forbidden]]) {
        if (column < 0 || cellValue(row[column]) !== cellValue(expected)) throw new Error(`${path}: human table disagrees with ${entry.conceptId}.${field}`);
      }
    }
  }
  if (block.sourceTable) for (const entry of entries.filter((candidate) => candidate.locale === "en-US")) {
    if (!englishMatched.has(entry.conceptId)) throw new Error(`${path}: explicit English source table mapping for ${entry.conceptId} is missing`);
  }
  for (const entry of entries) if (entry.sourceRow && !matched.has(entry)) throw new Error(`${path}: missing mapped human table heading for ${entry.conceptId}`);
}

export function validateTerms(data) {
  if (!data || data.schemaVersion !== SCHEMA_VERSION || !Array.isArray(data.entries)) throw new Error("terms data must use schemaVersion=1 and an entries array");
  if (typeof data.sourceHash !== "string" || !/^[a-f0-9]{64}$/.test(data.sourceHash)) throw new Error("terms data must contain a sha256 sourceHash");
  const canonical = new Set(data.entries.filter((entry) => entry.locale === "en-US").map((entry) => entry.conceptId));
  if (!canonical.size) throw new Error("terms data must contain canonical en-US entries");
  const seen = new Set();
  for (const entry of data.entries) {
    if (!entry || typeof entry !== "object") throw new Error("term entry must be an object");
    for (const field of ["conceptId", "locale", "preferred", "allowed", "forbidden", "mode", "scope", "exceptions", "sourceAnchor"]) {
      if (entry[field] === undefined) throw new Error(`term ${entry.conceptId ?? "<unknown>"}: missing ${field}`);
    }
    if (typeof entry.conceptId !== "string" || !entry.conceptId.trim()) throw new Error("term: conceptId must be a non-empty string");
    if (!LOCALES.has(entry.locale)) throw new Error(`${entry.conceptId}: unknown locale ${entry.locale}`);
    if (entry.locale !== "en-US" && !canonical.has(entry.conceptId)) throw new Error(`${entry.conceptId}: unknown canonical ID`);
    if (!MODES.has(entry.mode)) throw new Error(`${entry.conceptId}: unknown presentation mode ${entry.mode}`);
    if (typeof entry.preferred !== "string" || !entry.preferred.trim()) throw new Error(`${entry.conceptId}: preferred must be a non-empty string`);
    if (!Array.isArray(entry.allowed) || !Array.isArray(entry.forbidden) || !Array.isArray(entry.scope) || !Array.isArray(entry.exceptions)) throw new Error(`${entry.conceptId}: allowed, forbidden, scope, and exceptions must be arrays`);
    if ([...entry.allowed, ...entry.forbidden, ...entry.scope, ...entry.exceptions].some((value) => typeof value !== "string")) throw new Error(`${entry.conceptId}: term lists must contain strings`);
    if (typeof entry.sourceAnchor !== "string" || !entry.sourceAnchor.trim() || !/^[-\w./]+(?::\d+|#[^\s]+)$/.test(entry.sourceAnchor.trim())) throw new Error(`${entry.conceptId}: missing source anchor or invalid source anchor`);
    if (entry.sourceRow !== undefined) {
      if (!entry.sourceRow || typeof entry.sourceRow !== "object" || !["heading", "term", "mode", "preferred", "forbidden"].every((field) => typeof entry.sourceRow[field] === "string" && entry.sourceRow[field].trim())) throw new Error(`${entry.conceptId}: sourceRow must identify heading, term, mode, preferred, and forbidden cells`);
    }
    const allowed = new Set(list(entry.allowed));
    const forbidden = new Set(list(entry.forbidden));
    if ([...allowed].some((form) => forbidden.has(form))) throw new Error(`${entry.conceptId}: form is both allowed and forbidden`);
    if (forbidden.has(entry.preferred)) throw new Error(`${entry.conceptId}: preferred form is forbidden`);
    const key = entryKey(entry);
    if (seen.has(key)) throw new Error(`duplicate term ${key}`);
    seen.add(key);
  }
  const byLocale = new Map();
  for (const entry of data.entries) {
    if (!byLocale.has(entry.locale)) byLocale.set(entry.locale, []);
    byLocale.get(entry.locale).push(entry);
  }
  for (const [locale, entries] of byLocale) {
    const approved = new Map();
    for (const entry of entries) {
      for (const form of new Set([entry.preferred, ...entry.allowed])) {
        if (!form) continue;
        if (!approved.has(form)) approved.set(form, []);
        approved.get(form).push(entry.conceptId);
      }
    }
    for (const entry of entries) {
      for (const form of entry.forbidden) {
        const owners = approved.get(form) ?? [];
        if (owners.length) throw new Error(`${locale}: form “${form}” is approved by ${owners.join(", ")} but forbidden by ${entry.conceptId}`);
      }
    }
  }
  return data;
}

export function buildTerms(sources) {
  const parts = sourceParts(sources).sort((a, b) => a.path.localeCompare(b.path));
  const parsed = parts.flatMap(({ path, text }) => {
    const parsedBlocks = blocks(text, path);
    for (const block of parsedBlocks) checkHumanTables(text, list(block.entries), path, block);
    return parsedBlocks;
  });
  const canonical = parsed.find((block) => block.locale === "en-US" || block.authority === "glossary");
  if (!canonical) throw new Error("missing canonical glossary term block");
  const entries = parsed.flatMap((block) => list(block.entries).map((entry) => ({
    conceptId: entry.conceptId,
    locale: entry.locale ?? block.locale ?? "en-US",
    preferred: entry.preferred,
    allowed: list(entry.allowed),
    forbidden: list(entry.forbidden),
    mode: entry.mode,
    scope: list(entry.scope ?? ["prose"]),
    exceptions: list(entry.exceptions),
    sourceAnchor: entry.sourceAnchor,
    ...(entry.sourceRow ? { sourceRow: entry.sourceRow } : {}),
  })));
  const completeRoster = parsed.some((block) => block.completeRoster === true);
  if (completeRoster) {
    const canonicalIds = new Set(entries.filter((entry) => entry.locale === "en-US").map((entry) => entry.conceptId));
    for (const locale of ["zh-TW", "ja-JP"]) {
      const present = new Set(entries.filter((entry) => entry.locale === locale).map((entry) => entry.conceptId));
      for (const conceptId of canonicalIds) {
        if (!present.has(conceptId)) throw new Error(`missing explicit ${locale} entry for canonical ID ${conceptId}`);
      }
    }
  }
  const data = { schemaVersion: SCHEMA_VERSION, sourceHash: hash(parts.map(({ path, text }) => `${path}\n${text}`).join("\n")), entries: entries.sort((a, b) => entryKey(a).localeCompare(entryKey(b))) };
  return validateTerms(data);
}

export async function loadSources(root = process.cwd()) {
  return Object.fromEntries(await Promise.all(AUTHORITY_PATHS.map(async (path) => [path, await readFile(resolve(root, path), "utf8")] )));
}

export async function generate(root = process.cwd()) {
  const sources = await loadSources(root);
  const data = buildTerms(sources);
  const target = snapshotPath(root, data.sourceHash);
  await mkdir(dirname(target), { recursive: true });
  try {
    const current = validateTerms(JSON.parse(await readFile(target, "utf8")));
    if (JSON.stringify(current) !== JSON.stringify(data)) throw new Error(`invalid existing terminology snapshot at ${target}`);
    return data;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const temporary = `${target}.${process.pid}.${randomUUID()}.tmp`;
  await writeFile(temporary, `${JSON.stringify(data, null, 2)}\n`, "utf8");
  try { await link(temporary, target); }
  catch (error) {
    if (error.code !== "EEXIST") throw error;
    const current = validateTerms(JSON.parse(await readFile(target, "utf8")));
    if (JSON.stringify(current) !== JSON.stringify(data)) throw new Error(`invalid existing terminology snapshot at ${target}`);
  } finally { await unlink(temporary).catch(() => {}); }
  return data;
}

export async function check(root = process.cwd()) {
  const expected = buildTerms(await loadSources(root));
  let actual;
  const path = snapshotPath(root, expected.sourceHash);
  try { actual = JSON.parse(await readFile(path, "utf8")); } catch { throw new Error(`missing derived terminology data at ${path}`); }
  validateTerms(actual);
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error("derived terminology data is stale or non-deterministic");
  return actual;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try { if (process.argv.includes("--check")) await check(); else await generate(); }
  catch (error) { console.error(`terms: ${error.message}`); process.exitCode = 2; }
}
