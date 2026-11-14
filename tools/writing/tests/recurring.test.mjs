import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync, renameSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const checker = resolve("tools/writing/check-recurring.mjs");
function fixture(fn) {
  const cwd = mkdtempSync(join(tmpdir(), "recurring-"));
  const git = (...args) => execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
  try {
    git("init", "-q"); git("config", "core.hooksPath", join(cwd, "no-hooks")); git("config", "user.email", "test@example.com"); git("config", "user.name", "Test");
    const commit = (text) => { writeFileSync(join(cwd, "sample.md"), text); git("add", "-A"); git("commit", "-qm", "fixture"); return git("rev-parse", "HEAD"); };
    const run = (...args) => spawnSync(process.execPath, [checker, ...args], { cwd, encoding: "utf8" });
    fn({ cwd, git, commit, run });
  } finally { rmSync(cwd, { recursive: true, force: true }); }
}
function compare(before, after, expected, rule) {
  fixture(({ commit, run }) => {
    const base = commit(before); const head = commit(after);
    const result = run("--base", base, "--head", head);
    assert.equal(result.status, expected, result.stderr);
    if (rule) assert.match(result.stderr, new RegExp(`sample\\.md:1: ${rule}`));
  });
}

test("historical semicolon sentences and their fixes", () => {
  const cases = [
    ["- **Repo-Owned Files as Durable State**: The repository-owned `.dev/` directory, plan files, and research artifacts serve as the single source of truth for session continuity across tools; transient runtime caches must not replace these files.", "- **Repo-Owned Files as Durable State**: The repository-owned `.dev/` directory, plan files, and research artifacts serve as the single source of truth for session continuity across tools. Transient runtime caches must not replace these files."],
    ["**Step 3 — verify (automated; NOT RUN).**", "**Step 3 — verify (automated, NOT RUN).**"],
  ];
  for (const [bad, fixed] of cases) { compare("old\n", `${bad}\n`, 1, "prose-semicolon"); compare("old\n", `${fixed}\n`, 0); }
});

test("historical Git operation sentences and fixes", () => {
  const cases = [
    "- **與 c1 config 遷移交疊**(Med,新接地)— flag 住 `config.json`;c1 正遷 reader 至 `config.json` 並動 `health.rs`。本計畫沿用 c1 raw-Value reader 策略;`health.rs` task 與 c1 T" + "-14 排序對齊(後落地者 rebase)。",
    "- **與姊妹計畫同改 planning SKILL**:`infra-planning-flow-mechanization` 也碰 `commands/{planning,deep-planning,refining-plan}` SKILL。緩解:本計畫只在「重複方法 prose」處改 pointer,與機制化的 receipt-gate 改動是不同段落;若同時在飛,後 land 者 rebase。",
  ];
  for (const bad of cases) { compare("old\n", `${bad}\n`, 1, "git-integration"); compare("old\n", "Use `git merge`.\n", 0); }
});

test("prose protection and rule markers", () => {
  for (const text of ["prose; text", "prose； text", "**bold; text**"]) compare("old\n", `${text}\n`, 1, "prose-semicolon");
  for (const text of ["`code;`", "https://example.com/a;b", "```js\nconst x = 1;\n```", "> quoted; text"]) compare("old\n", `${text}\n`, 0);
  for (const term of ["squash", "rebase", "--ff-only", "--no-ff"]) compare("old\n", `Use \`${term}\`.\n`, 1, "git-integration");
  compare("old\n", "Use rebase. <!-- allow: git-integration -->\n", 0);
  compare("old\n", "Use rebase; carefully. <!-- allow: git-integration -->\n", 1, "prose-semicolon");
  compare("old\n", "Allowed; text. <!-- allow: prose-semicolon -->\n", 0);
});

test("only added lines are checked, including edits and deletions", () => {
  compare("old; violation\n", "old; violation\nnew text\n", 0);
  compare("old; violation\n", "edited; violation\n", 1, "prose-semicolon");
  compare("old; violation\n", "new; violation\n", 1, "prose-semicolon");
  fixture(({ cwd, git, commit, run }) => {
    const base = commit("old; violation\n");
    renameSync(join(cwd, "sample.md"), join(cwd, "renamed.md")); git("add", "-A"); git("commit", "-qm", "rename");
    assert.equal(run("--base", base).status, 0);
    const renamed = git("rev-parse", "HEAD"); rmSync(join(cwd, "renamed.md")); git("add", "-A"); git("commit", "-qm", "delete");
    assert.equal(run("--base", renamed).status, 0);
  });
});

test("bad inputs and unrelated histories", () => {
  fixture(({ git, commit, run }) => {
    const base = commit("first\n");
    assert.equal(run("--base", "missing-object").status, 2);
    assert.equal(run("--unknown").status, 2);
    git("branch", "-M", "fixture");
    assert.equal(run().status, 2);
    git("checkout", "--orphan", "other");
    const head = commit("new; text\n");
    const result = run("--base", base, "--head", head);
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, /prose-semicolon/);
  });
});

test("GitHub event revisions select the same changes as direct arguments", () => {
  fixture(({ cwd, commit, run }) => {
    const base = commit("clean text\n");
    const head = commit("new; violation\n");
    const eventPath = join(cwd, "event.json");
    const direct = run("--base", base, "--head", head);
    for (const event of [
      { pull_request: { base: { sha: base }, head: { sha: head } } },
      { before: base, after: head },
    ]) {
      writeFileSync(eventPath, JSON.stringify(event));
      const result = run("--event", eventPath);
      assert.equal(result.status, direct.status);
      assert.equal(result.stderr, direct.stderr);
    }
    for (const before of ["0".repeat(40), undefined]) {
      writeFileSync(eventPath, JSON.stringify({ before, after: head }));
      const result = run("--event", eventPath);
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /Skipping recurring mistakes check/);
    }
    assert.equal(run("--event", eventPath, "--base", base).status, 2);
    writeFileSync(eventPath, JSON.stringify({ pull_request: {} }));
    assert.equal(run("--event", eventPath).status, 2);
  });
});
