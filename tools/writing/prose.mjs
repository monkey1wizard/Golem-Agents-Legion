import { parse, Syntax } from "@textlint/markdown-to-ast";

const UTF8_ENCODER = new TextEncoder();

/**
 * Convert a JavaScript string offset (UTF-16 code units) to a UTF-8 byte
 * offset. Textlint ranges use JavaScript offsets; byte offsets are exposed
 * only for callers that need to cross a byte-oriented boundary.
 */
export function utf16ToUtf8Offset(text, offset) {
  return UTF8_ENCODER.encode(text.slice(0, offset)).byteLength;
}

export function utf8ToUtf16Offset(text, byteOffset) {
  let bytes = 0;
  let offset = 0;
  for (const character of text) {
    const size = UTF8_ENCODER.encode(character).byteLength;
    if (bytes + size > byteOffset) break;
    bytes += size;
    offset += character.length;
  }
  return offset;
}

const PROTECTED_NODES = new Set([
  Syntax.Code,
  Syntax.CodeBlock,
  Syntax.Html,
  Syntax.HtmlBlock,
  Syntax.Comment,
  Syntax.Definition,
  Syntax.ReferenceDef,
  Syntax.Image,
  Syntax.ImageReference,
  Syntax.BlockQuote,
]);

const MACHINE_LINE = /^\s*(?:<!--.*-->|\[![-\w]+\]|(?:receipt(?:s)?|path|file|id|anchor|hash)\s*[:=])/i;
const MACHINE_TOKEN = new RegExp(String.raw`(?:https?://|ftp://|mailto:|www\.)[^\s<>\x60]+|(?:[A-Za-z]:[\\/]|\.{1,2}[\\/]|(?:[A-Za-z0-9._-]+[\\/])+)[^\s<>\x60]+|(?:^|[\s([{\"'\x60])(?:node|npm|npx|cargo|git|gal|pnpm|yarn|deno|python|bash|sh|pwsh|powershell)(?=\s|$)|(?:^|[\s([{\"'\x60])--?[A-Za-z][\w-]*`, "g");
const MACHINE_ID = /\b(?:T|TP|R|C|ID)-\d{1,3}\b|\b[a-f0-9]{7,64}\b|\.dev\/pipeline\/[^\s)]+/gi;
const VERBATIM_LINE = /^\s*>/;

function rangeOf(node) {
  return Array.isArray(node?.range) && node.range.length === 2 ? node.range : null;
}

function textOf(node, source) {
  const range = rangeOf(node);
  return range ? source.slice(range[0], range[1]) : "";
}

function isDesignatedCell(cell, headerNames) {
  const index = cell.parent?.children?.indexOf(cell);
  if (index === undefined || index < 0) return false;
  const header = headerNames[index] ?? "";
  return /^(?:term|terms|example|examples|forbidden|forbid|allowed|preferred|aliases|literal|source|禁止寫法|避ける表記)$/i.test(header.trim());
}

function collectTableHeaders(table, source) {
  const firstRow = table.children?.[0];
  return (firstRow?.children ?? []).map((cell) => textOf(cell, source).replace(/^\s*\|?\s*|\s*\|?\s*$/g, ""));
}

function splitProtectedTokens(start, end, source) {
  const value = source.slice(start, end);
  const protectedRanges = [];
  for (const expression of [MACHINE_TOKEN, MACHINE_ID]) {
    expression.lastIndex = 0;
    let match;
    while ((match = expression.exec(value))) {
      const token = match[0];
      const leading = token.match(/^[\s([{\"'`]/)?.[0].length ?? 0;
      const tokenStart = start + match.index + leading;
      const tokenEnd = start + match.index + token.length;
      if (tokenEnd > tokenStart) protectedRanges.push([tokenStart, tokenEnd]);
      if (expression.lastIndex <= match.index) expression.lastIndex = match.index + 1;
    }
  }
  return protectedRanges;
}

function subtractRanges(range, protectedRanges) {
  let pieces = [range];
  for (const [protectedStart, protectedEnd] of protectedRanges) {
    pieces = pieces.flatMap(([start, end]) => {
      if (protectedEnd <= start || protectedStart >= end) return [[start, end]];
      return [
        ...(protectedStart > start ? [[start, protectedStart]] : []),
        ...(protectedEnd < end ? [[protectedEnd, end]] : []),
      ];
    });
  }
  return pieces.filter(([start, end]) => end > start);
}

function visit(node, source, state, ancestors = []) {
  if (!node) return;
  const range = rangeOf(node);
  const nodeText = range ? textOf(node, source) : "";
  if (range && (PROTECTED_NODES.has(node.type) || VERBATIM_LINE.test(nodeText))) {
    state.protected.push(range);
    return;
  }
  if (node.type === "Yaml" || node.type === "FrontMatter") {
    if (range) state.protected.push(range);
    return;
  }
  if (node.type === Syntax.Table) {
    const headers = collectTableHeaders(node, source);
    node.children?.forEach((row, rowIndex) => row.children?.forEach((cell) => {
      cell.parent = row;
      if (isDesignatedCell(cell, headers)) {
        if (rangeOf(cell)) state.protected.push(rangeOf(cell));
      } else visit(cell, source, state, [...ancestors, node, row]);
    }));
    return;
  }
  if (node.type === Syntax.TableRow || node.type === Syntax.TableCell) {
    node.children?.forEach((child) => visit(child, source, state, [...ancestors, node]));
    return;
  }
  if (node.type === Syntax.Str && range) {
    const lineStart = source.lastIndexOf("\n", range[0] - 1) + 1;
    const lineEnd = source.indexOf("\n", range[1]);
    const line = source.slice(lineStart, lineEnd < 0 ? source.length : lineEnd);
    if (MACHINE_LINE.test(line)) {
      state.protected.push(range);
      return;
    }
    const protectedTokens = splitProtectedTokens(range[0], range[1], source);
    state.protected.push(...protectedTokens);
    state.prose.push(...subtractRanges(range, protectedTokens));
    return;
  }
  node.children?.forEach((child) => visit(child, source, state, [...ancestors, node]));
}

/**
 * Return UTF-16 source ranges containing prose. Ranges are disjoint and use
 * the original Markdown offsets, so lint findings map back without rewriting.
 */
export function proseRanges(text, filename = "") {
  if (typeof text !== "string") throw new TypeError("text must be a string");
  const ast = parse(text);
  const state = { prose: [], protected: [] };
  visit(ast, text, state);
  const ranges = state.prose
    .sort((a, b) => a[0] - b[0] || a[1] - b[1])
    .filter(([start, end]) => end > start);
  return ranges;
}

/** Native textlint filter-rule reporter. */
export function filter(context) {
  return {
    [Syntax.Document](node) {
      const source = context.getSource(node);
      const filename = context.getFilePath() ?? "";
      const prose = proseRanges(source, filename);
      const all = [[0, source.length]];
      const protectedRanges = subtractRanges(all[0], prose);
      for (const range of protectedRanges) context.shouldIgnore(range, {});
    },
  };
}

export default filter;
