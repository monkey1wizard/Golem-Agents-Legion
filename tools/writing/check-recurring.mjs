import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { proseRanges } from "./prose.mjs";

function git(...args) {
  return execFileSync("git", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "pipe"] });
}

function revision(value) {
  return git("rev-parse", "--verify", "--end-of-options", `${value}^{commit}`).trim();
}

function main(args) {
  let base;
  let head = "HEAD";
  let eventPath;
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const option = args[i];
    if (!['--base', '--head', '--event'].includes(option) || seen.has(option) || !args[i + 1] || args[i + 1].startsWith("--")) throw new Error("expected [--base BASE] [--head HEAD] or --event PATH");
    seen.add(option);
    if (option === "--base") base = args[++i];
    else if (option === "--head") head = args[++i];
    else eventPath = args[++i];
  }
  if (eventPath) {
    if (seen.has("--base") || seen.has("--head")) throw new Error("--event cannot be combined with --base or --head");
    const event = JSON.parse(readFileSync(eventPath, "utf8"));
    if (!event || typeof event !== "object" || Array.isArray(event)) throw new Error("invalid event JSON");
    if (event.pull_request) {
      base = event.pull_request.base?.sha;
      head = event.pull_request.head?.sha;
    } else {
      base = event.before;
      head = event.after;
      if (base == null || base === "" || (typeof base === "string" && /^0+$/.test(base))) {
        console.log("Skipping recurring mistakes check: push event has no previous revision.");
        return 0;
      }
    }
    if (typeof base !== "string" || !base || typeof head !== "string" || !head) throw new Error("event is missing revisions");
  }
  head = revision(head);
  base = base ? revision(base) : git("merge-base", "main", head).trim();
  const entries = git("diff", "--name-status", "-z", "--diff-filter=ACMR", "-M", base, head).split("\0").filter(Boolean);
  const paths = [];
  while (entries.length) {
    const status = entries.shift();
    const oldPath = entries.shift();
    const path = /^[RC]/.test(status) ? entries.shift() : oldPath;
    if (/\.(?:md|markdown)$/i.test(path)) paths.push({ path, oldPath });
  }
  let violations = 0;
  for (const { path, oldPath } of paths) {
    const source = git("show", `${head}:${path}`);
    const ranges = proseRanges(source, path);
    const lines = source.split("\n");
    const offsets = [];
    let offset = 0;
    for (const line of lines) { offsets.push(offset); offset += line.length + 1; }
    const diff = git("--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", "-U0", "-M", base, head, "--", oldPath, path);
    let lineNumber = 0;
    let inHunk = false;
    for (const line of diff.split("\n")) {
      const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (hunk) { lineNumber = Number(hunk[1]); inHunk = true; continue; }
      if (!inHunk) continue;
      if (line.startsWith("+")) {
        const content = lines[lineNumber - 1] ?? "";
        if (!content.includes("<!-- allow: prose-semicolon -->")) {
          const start = offsets[lineNumber - 1];
          const found = [...content.matchAll(/[;；]/g)].some((match) => ranges.some(([a, b]) => start + match.index >= a && start + match.index < b));
          if (found) { console.error(`${path}:${lineNumber}: prose-semicolon: replace the semicolon with a period or other suitable punctuation`); violations++; }
        }
        if (!content.includes("<!-- allow: git-integration -->") && /\b(?:squash|rebase)\b|--(?:ff-only|no-ff)\b/i.test(content)) {
          console.error(`${path}:${lineNumber}: git-integration: use plain git merge`); violations++;
        }
        lineNumber++;
      } else if (line.startsWith(" ")) lineNumber++;
    }
  }
  return violations ? 1 : 0;
}

try { process.exitCode = main(process.argv.slice(2)); }
catch (error) { console.error(`input error: ${error.message}`); process.exitCode = 2; }
