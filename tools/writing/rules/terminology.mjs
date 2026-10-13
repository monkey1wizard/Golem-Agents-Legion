import { readFileSync } from "node:fs";
import { AUTHORITY_PATHS, buildTerms, validateTerms, snapshotPath } from "../terms.mjs";
import { resolve } from "node:path";
import { proseRanges } from "../prose.mjs";

const OPERATIONAL_RULE = "GAL/terminology-operational";
const RULE_ID = "GAL/terminology";

function loadData() {
  try {
    const root = process.cwd();
    const sources = Object.fromEntries(AUTHORITY_PATHS.map((path) => [path, readFileSync(resolve(root, path), "utf8")]));
    const expected = buildTerms(sources);
    const pin = process.env.GAL_WRITING_TERMS_HASH;
    if (pin !== undefined && pin !== expected.sourceHash) throw new Error("terminology source hash does not match parent pin");
    const path = snapshotPath(root, expected.sourceHash);
    const data = validateTerms(JSON.parse(readFileSync(path, "utf8")));
    if (JSON.stringify(data) !== JSON.stringify(expected)) throw new Error("derived terminology data does not match authority files");
    return data;
  }
  catch (error) {
    const failure = new Error(`terminology data unavailable or invalid: ${error.message}`);
    failure.ruleId = OPERATIONAL_RULE;
    failure.severity = 2;
    throw failure;
  }
}

function report(context, node, message, index, length, entry, severity = 2, replacement = null) {
  const configuredSeverity = context.severity ?? severity;
  const level = configuredSeverity === 2 ? "error" : "advisory";
  const detailedMessage = `${RULE_ID} [${level}] [source: ${entry.sourceAnchor}] ${message}`;
  // In pinned textlint 15.2.3, RuleError discards custom fields. A reported
  // object preserves them in message.data. Its severity must be explicit.
  const finding = {
    message: detailedMessage,
    severity: configuredSeverity,
    ruleId: RULE_ID,
    sourceAnchor: entry.sourceAnchor,
    suggestion: { text: `${message} Check the authority and preserve the intended meaning before editing.`, replacement, applicability: "manual" },
  };
  if (context.locator) finding.padding = context.locator.range([index, index + length]);
  else Object.assign(finding, { index, length });
  context.report(node, finding);
}

function inScope(entry, filePath) {
  return entry.scope.some((scope) => scope === "prose" || scope === filePath || (scope.startsWith("*.") && filePath.endsWith(scope.slice(1))));
}

function isException(source, start, form, exceptions) {
  return exceptions.some((exception) => {
    if (!exception) return false;
    const windowStart = Math.max(0, start - exception.length + form.length);
    const windowEnd = Math.min(source.length, start + form.length + exception.length);
    const window = source.slice(windowStart, windowEnd);
    return window.includes(exception) && exception.includes(form);
  });
}

function hasLatinBoundary(source, start, end, form) {
  if (!/[A-Za-z0-9]/.test(form)) return true;
  const before = source[start - 1] ?? "";
  const after = source[end] ?? "";
  return !/[A-Za-z0-9_]/.test(before) && !/[A-Za-z0-9_]/.test(after);
}

function eligibleOccurrences(source, ranges, form, entry, filePath) {
  if (!inScope(entry, filePath)) return [];
  const result = [];
  for (const [start, end] of ranges) {
    const segment = source.slice(start, end);
    let offset = segment.indexOf(form);
    while (offset >= 0) {
      const absolute = start + offset;
      const finish = absolute + form.length;
      if (hasLatinBoundary(source, absolute, finish, form) && !isException(source, absolute, form, entry.exceptions)) result.push(absolute);
      offset = segment.indexOf(form, offset + Math.max(form.length, 1));
    }
  }
  return result;
}

function occurrenceRanges(source, ranges, form, entry, filePath) {
  return eligibleOccurrences(source, ranges, form, entry, filePath).map((start) => [start, start + form.length]);
}

function makeRule(context, options, getData) {
  const channel = options.channel ?? "hard";
  if (!["hard", "advisory", "all"].includes(channel)) throw new Error(`Unknown terminology channel: ${channel}`);
  const emit = (...args) => {
    const severity = args[6] ?? 2;
    if (channel === "all" || channel === (severity === 2 ? "hard" : "advisory")) report(...args);
  };
  return {
    [context.Syntax.Document](node) {
      const data = getData();
      const source = context.getSource(node);
      const filePath = context.getFilePath?.() ?? "";
      const ranges = proseRanges(source, filePath);
      const locale = options.locale ?? "en-US";
      const entries = data.entries.filter((entry) => entry.locale === locale);
      const approvedRanges = entries.flatMap((entry) => [entry.preferred, ...entry.allowed]
        .filter(Boolean)
        .flatMap((form) => occurrenceRanges(source, ranges, form, entry, filePath)));
      for (const entry of entries) {
          const forbidden = entry.forbidden.filter(Boolean);
          for (const form of forbidden) {
            for (const absolute of eligibleOccurrences(source, ranges, form, entry, filePath)) {
              const forbiddenEnd = absolute + form.length;
              if (approvedRanges.some(([approvedStart, approvedEnd]) => approvedStart <= absolute && forbiddenEnd <= approvedEnd && (approvedStart < absolute || approvedEnd > forbiddenEnd))) continue;
              emit(context, node, `Forbidden form “${form}” for ${entry.conceptId}; use “${entry.preferred}”.`, absolute, form.length, entry, entry.mode === "contextual" ? 1 : 2, entry.preferred);
            }
          }
          if (entry.mode === "bilingual-first-use" && entry.preferred) {
            const first = eligibleOccurrences(source, ranges, entry.preferred, entry, filePath)[0];
            const bilingual = entry.allowed.find((value) => value !== entry.preferred && value.includes(entry.preferred));
            if (first !== undefined && bilingual && !eligibleOccurrences(source, ranges, bilingual, entry, filePath).includes(first)) emit(context, node, `First use of ${entry.conceptId} must use “${bilingual}”.`, first, entry.preferred.length, entry, 1, bilingual);
          }
          if (entry.mode === "contextual" && entry.preferred && source.includes(entry.preferred)) {
            const first = eligibleOccurrences(source, ranges, entry.preferred, entry, filePath)[0];
            if (first !== undefined) emit(context, node, `Context determines the presentation of ${entry.conceptId}; review this occurrence.`, first, entry.preferred.length, entry, 1);
          }
      }
    },
  };
}

export default function terminology(context, options = {}) {
  if (options.data !== undefined || options.testOnly !== undefined) throw new Error("Inline terminology data is not a production option");
  if (options.channel === "all") throw new Error("Configure separate hard/error and advisory/warning terminology instances");
  return makeRule(context, options, loadData);
}

/** Construct an isolated rule for tests without changing the production loader. */
export function createTerminologyRule(data, options = {}) {
  const validated = validateTerms(data);
  return (context) => makeRule(context, options, () => validated);
}

export { loadData, OPERATIONAL_RULE, RULE_ID };
