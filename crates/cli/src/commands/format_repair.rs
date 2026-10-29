//! Internal, fail-closed document format repair helper.

use super::finalize_check::{
    parse_receipt_envelope, receipt_envelope, validate_repair_context, RepairContext, RepairTarget,
};
use gal_engine::ExitCode;
use sha2::{Digest, Sha256};
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Bring a linked checkout forward to the primary checkout's captured branch tip
/// before an eligible pipeline entry binds its executable.
pub(crate) fn recover_primary_tip(root: &Path, prompt: &Path) -> Result<bool, String> {
    recover_primary_tip_inner(
        root,
        prompt,
        #[cfg(test)]
        None,
    )
}

#[cfg(test)]
struct RecoveryTestHooks<'a> {
    after_tip_captured: &'a dyn Fn(&Path, &str),
    abort_failure: Option<&'a str>,
}

fn recover_primary_tip_inner(
    root: &Path,
    prompt: &Path,
    #[cfg(test)] hooks: Option<&RecoveryTestHooks<'_>>,
) -> Result<bool, String> {
    let root = fs::canonicalize(root)
        .map_err(|error| format!("cannot resolve linked worktree: {error}"))?;
    let run = |cwd: &Path, args: &[&str]| -> Result<String, String> {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .map_err(|error| format!("cannot run git {}: {error}", args.join(" ")))?;
        if !output.status.success() {
            return Err(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let git_path = |arg: &str| -> Result<PathBuf, String> {
        let value = run(&root, &["rev-parse", arg])?;
        let path = PathBuf::from(value);
        fs::canonicalize(if path.is_absolute() {
            path
        } else {
            root.join(path)
        })
        .map_err(|error| format!("cannot resolve git directory: {error}"))
    };
    if git_path("--absolute-git-dir")? == git_path("--git-common-dir")? {
        return Ok(false);
    }
    let prompt = if prompt.exists() {
        fs::canonicalize(prompt).map_err(|error| format!("cannot resolve entry prompt: {error}"))?
    } else {
        let mut ancestor = prompt;
        let mut suffix = Vec::new();
        while !ancestor.exists() {
            suffix.push(
                ancestor
                    .file_name()
                    .ok_or("entry prompt has no file name")?
                    .to_os_string(),
            );
            ancestor = ancestor
                .parent()
                .ok_or("entry prompt has no existing parent")?;
        }
        let canonical_parent = fs::canonicalize(ancestor)
            .map_err(|error| format!("cannot resolve entry prompt parent: {error}"))?;
        if !canonical_parent.starts_with(&root) {
            return Err("entry prompt parent resolves outside the linked worktree".into());
        }
        suffix
            .into_iter()
            .rev()
            .fold(canonical_parent, |path, part| path.join(part))
    };
    let status = |cwd: &Path| -> Result<(), String> {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(["status", "--porcelain", "--untracked-files=all"])
            .output()
            .map_err(|error| format!("cannot inspect git worktree: {error}"))?;
        if !output.status.success() {
            return Err("cannot inspect git worktree status".into());
        }
        if !output.stdout.is_empty() {
            return Err(format!(
                "worktree is dirty: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ));
        }
        Ok(())
    };

    status(&root)?;
    let head = run(&root, &["rev-parse", "HEAD"])?;
    let original_index = run(&root, &["write-tree"])?;
    let common = run(&root, &["rev-parse", "--git-common-dir"])?;
    let common = fs::canonicalize(root.join(common))
        .map_err(|error| format!("cannot resolve common git directory: {error}"))?;
    let primary = common
        .parent()
        .ok_or("common git directory has no checkout parent")?;
    let branch = run(primary, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_err(|_| "primary checkout is detached or has no branch".to_string())?;
    let ref_name = format!("refs/heads/{branch}");
    let tip = run(primary, &["rev-parse", &ref_name])?;
    status(primary)?;
    #[cfg(test)]
    if let Some(hooks) = hooks {
        (hooks.after_tip_captured)(primary, &ref_name);
    }
    if run(primary, &["rev-parse", &ref_name])? != tip {
        return Err("primary branch tip moved during recovery".into());
    }

    let relative = prompt
        .strip_prefix(&root)
        .map_err(|_| "entry prompt is outside the linked worktree")?;
    if relative.components().any(|part| {
        matches!(
            part,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err("entry prompt path is not a safe worktree-relative path".into());
    }
    let prompt_name = relative.to_string_lossy().replace('\\', "/");
    let primary_tracks_prompt = Command::new("git")
        .current_dir(primary)
        .stderr(std::process::Stdio::null())
        .args(["cat-file", "-e"])
        .arg(format!("{tip}:{prompt_name}"))
        .status()
        .map_err(|error| format!("cannot check prompt at primary tip: {error}"))?
        .success();
    let current_tracks_prompt = Command::new("git")
        .current_dir(&root)
        .stderr(std::process::Stdio::null())
        .args(["cat-file", "-e"])
        .arg(format!("{head}:{prompt_name}"))
        .status()
        .map_err(|error| format!("cannot check prompt at linked tip: {error}"))?
        .success();
    // A linked-branch-only prompt remains valid, but does not exempt the
    // checkout from merging an advanced primary tip.
    if !primary_tracks_prompt && !current_tracks_prompt {
        return Err("prompt is absent from both the primary tip and linked HEAD".into());
    }
    if head == tip {
        return Ok(false);
    }
    let is_ancestor = |ancestor: &str, descendant: &str| -> Result<bool, String> {
        let output = Command::new("git")
            .current_dir(&root)
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .status()
            .map_err(|error| format!("cannot verify primary-tip ancestry: {error}"))?;
        Ok(output.success())
    };
    if is_ancestor(&tip, &head)? {
        return Ok(false);
    }
    if !is_ancestor(&head, &tip)? {
        let common_ancestor = Command::new("git")
            .current_dir(&root)
            .args(["merge-base", &head, &tip])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|error| format!("cannot verify shared history: {error}"))?;
        if !common_ancestor.success() {
            return Err("primary tip and linked HEAD do not share history".into());
        }
    }
    status(&root)?;
    if run(primary, &["rev-parse", &ref_name])? != tip {
        return Err("primary branch tip moved during recovery".into());
    }
    let merge = Command::new("git")
        .current_dir(&root)
        .args(["merge", "--no-edit", &tip])
        .output()
        .map_err(|error| format!("cannot merge captured primary tip: {error}"))?;
    if !merge.status.success() {
        let conflicts = run(&root, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        #[cfg(test)]
        let abort_result = if let Some(message) = hooks.and_then(|hooks| hooks.abort_failure) {
            Err(message.to_string())
        } else {
            Command::new("git")
                .current_dir(&root)
                .args(["merge", "--abort"])
                .output()
                .map(|output| output.status.success())
                .map_err(|error| error.to_string())
                .and_then(|success| {
                    success
                        .then_some(())
                        .ok_or_else(|| "git merge --abort failed".into())
                })
        };
        #[cfg(not(test))]
        let abort_result = Command::new("git")
            .current_dir(&root)
            .args(["merge", "--abort"])
            .output()
            .map(|output| output.status.success())
            .map_err(|error| error.to_string())
            .and_then(|success| {
                success
                    .then_some(())
                    .ok_or_else(|| "git merge --abort failed".into())
            });
        let restored = abort_result.is_ok()
            && run(&root, &["rev-parse", "HEAD"]).is_ok_and(|value| value == head)
            && run(&root, &["write-tree"]).is_ok_and(|value| value == original_index)
            && status(&root).is_ok();
        if restored {
            return Err(format!(
                "primary-tip merge conflicted and was aborted; conflicted paths: {}",
                if conflicts.is_empty() {
                    "unknown"
                } else {
                    &conflicts
                }
            ));
        }
        let residual = run(&root, &["status", "--porcelain", "--untracked-files=all"])
            .unwrap_or_else(|_| "status unavailable".into());
        return Err(format!(
            "primary-tip merge failed and restoration is unproven; conflicted paths: {}; residual paths: {}",
            if conflicts.is_empty() {
                "unknown"
            } else {
                &conflicts
            },
            if residual.is_empty() {
                "none"
            } else {
                &residual
            }
        ));
    }
    let merged_head = run(&root, &["rev-parse", "HEAD"])?;
    let ancestry = Command::new("git")
        .current_dir(&root)
        .args(["merge-base", "--is-ancestor", &tip, &merged_head])
        .status()
        .map_err(|error| format!("cannot verify merged ancestry: {error}"))?;
    if !ancestry.success()
        || !run(&root, &["cat-file", "-e", &format!("HEAD:{prompt_name}")]).is_ok()
    {
        return Err("post-merge ancestry or prompt tracking check failed".into());
    }
    status(&root)?;
    Ok(merged_head != head)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepairId {
    LeadingBom,
    HeadingLevel,
    BulletShape,
    TableShape,
    RetryFieldShape,
}

impl RepairId {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "leading-bom" => Ok(Self::LeadingBom),
            "heading-level" => Ok(Self::HeadingLevel),
            "bullet-shape" => Ok(Self::BulletShape),
            "table-shape" => Ok(Self::TableShape),
            "retry-field-shape" => Ok(Self::RetryFieldShape),
            _ => Err(format!("unsupported repair ID '{value}'")),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::LeadingBom => "leading-bom",
            Self::HeadingLevel => "heading-level",
            Self::BulletShape => "bullet-shape",
            Self::TableShape => "table-shape",
            Self::RetryFieldShape => "retry-field-shape",
        }
    }
}

fn lines_with_endings(bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "target is not UTF-8")?;
    Ok(text.split_inclusive('\n').map(str::to_owned).collect())
}

fn line_content(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn table_cells(line: &str) -> Option<Vec<&str>> {
    let value = line.trim();
    if !value.starts_with('|')
        || !value.ends_with('|')
        || value.contains("\\|")
        || value.contains('`')
    {
        return None;
    }
    Some(value[1..value.len() - 1].split('|').collect())
}

fn recognized_table_header(cells: &[&str]) -> bool {
    let normalized: Vec<String> = cells
        .iter()
        .map(|cell| cell.trim().to_ascii_lowercase())
        .collect();
    (normalized.iter().any(|cell| cell == "id") && normalized.iter().any(|cell| cell == "covers"))
        || normalized
            == [
                "id",
                "what to check",
                "expected result",
                "pass/fail rule",
                "why not automated",
            ]
        || normalized
            == [
                "id",
                "requirement",
                "check command",
                "expected result",
                "how to satisfy",
            ]
        || normalized == ["requirement", "l1", "l2", "l3", "l4"]
}

fn table_shape_candidate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut lines = lines_with_endings(bytes)?;
    let mut repaired = Vec::new();
    for index in 1..lines.len().saturating_sub(1) {
        let Some(header) = table_cells(line_content(&lines[index - 1])) else {
            continue;
        };
        let Some(delimiter) = table_cells(line_content(&lines[index])) else {
            continue;
        };
        let Some(data) = table_cells(line_content(&lines[index + 1])) else {
            continue;
        };
        if header.len() != delimiter.len() || header.len() != data.len() {
            continue;
        }
        if header.iter().any(|cell| cell.trim().is_empty()) || !recognized_table_header(&header) {
            continue;
        }
        let mut changed = false;
        let mut normalized = Vec::with_capacity(delimiter.len());
        let has_hyphen = delimiter.iter().any(|cell| cell.contains('-'));
        if !has_hyphen {
            return Err("table delimiter has no existing hyphen to normalize".into());
        }
        for cell in &delimiter {
            let raw = cell.trim();
            let left = raw.starts_with(':');
            let right = raw.ends_with(':');
            let core = raw.trim_matches(':');
            if !core.bytes().all(|byte| byte == b'-' || byte == b' ') {
                return Err("table delimiter contains unsupported or inferred cell".into());
            }
            let hyphens = core.chars().filter(|ch| *ch == '-').count();
            if hyphens == 0 {
                return Err("delimiter cell has no existing hyphen to normalize".into());
            }
            let body = "---".to_string();
            let cell = format!(
                "{}{}{}",
                if left { ":" } else { "" },
                body,
                if right { ":" } else { "" }
            );
            changed |= cell != raw;
            normalized.push(cell);
        }
        if changed {
            repaired.push(index);
            let newline = &lines[index][line_content(&lines[index]).len()..];
            let replacement = format!("| {} |{}", normalized.join(" | "), newline);
            lines[index] = replacement;
        }
    }
    if repaired.len() != 1 {
        return Err("table repair requires one unambiguous supported delimiter row".into());
    }
    Ok(lines.concat().into_bytes())
}

/// Read a Retry Handoff field label with an optional `- ` bullet and ASCII
/// label-case or spacing differences before the colon. `Status note` is
/// checked before `Status` so a note line never reads as a second status.
fn retry_field_label(content: &str) -> Option<(&'static str, &str)> {
    let trimmed = content.trim_start();
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    for label in ["Status note", "Status", "Problem", "Next human step"] {
        let Some(head) = body.get(..label.len()) else {
            continue;
        };
        if !head.eq_ignore_ascii_case(label) {
            continue;
        }
        if let Some(value) = body[label.len()..]
            .trim_start_matches(' ')
            .strip_prefix(':')
        {
            return Some((label, value));
        }
    }
    None
}

/// The single Retry Handoff block a repair may touch. When `Current Task`
/// names an active task, only that task's headings count.
fn retry_block_range(lines: &[String]) -> Result<(usize, usize), String> {
    let text = lines.concat();
    let active = match super::converge_check::current_task_value(&text) {
        super::converge_check::CurrentTaskValue::Active(task) => Some(task),
        _ => None,
    };
    let headings: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let rest = line_content(line)
                .trim()
                .strip_prefix("#### Retry Handoff — ")?;
            let (id, phase) = rest.rsplit_once(" / ")?;
            if !matches!(phase, "IMPLEMENT" | "TEST" | "AUDIT") {
                return None;
            }
            if active.as_deref().is_some_and(|task| task != id) {
                return None;
            }
            Some(index)
        })
        .collect();
    if headings.len() != 1 {
        return Err("retry repair requires one recognized Retry Handoff".into());
    }
    let start = headings[0] + 1;
    let end = (start..lines.len())
        .find(|index| {
            let content = line_content(&lines[*index]).trim_start();
            content.starts_with("#### ")
                || content.starts_with("### ")
                || content.starts_with("## ")
        })
        .unwrap_or(lines.len());
    Ok((start, end))
}

fn retry_field_candidate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut lines = lines_with_endings(bytes)?;
    let (start, end) = retry_block_range(&lines)?;
    let mut found = std::collections::BTreeSet::new();
    let mut inline_note = false;
    let mut changed = false;
    for line in lines.iter_mut().take(end).skip(start) {
        let content = line_content(line).to_string();
        let Some((label, value)) = retry_field_label(&content) else {
            continue;
        };
        if !found.insert(label) {
            return Err("duplicate Retry Handoff field".into());
        }
        let newline = line[content.len()..].to_string();
        let normalized = match label {
            "Status" => {
                let value = value.trim_start();
                if let Some((status, suffix)) = value.split_once(" — ") {
                    if !matches!(status, "OPEN" | "RESOLVED") || suffix.trim().is_empty() {
                        return Err("unknown or empty Retry Handoff status note".into());
                    }
                    inline_note = true;
                    format!("- Status: {status}{newline}- Status note: — {suffix}{newline}")
                } else if matches!(value, "OPEN" | "RESOLVED") {
                    format!("- Status: {value}{newline}")
                } else {
                    return Err("unknown Retry Handoff status".into());
                }
            }
            "Status note" => format!("- Status note:{value}{newline}"),
            _ => {
                let value = value.strip_prefix(' ').unwrap_or(value);
                format!("- {label}: {value}{newline}")
            }
        };
        if normalized != *line {
            changed = true;
            *line = normalized;
        }
    }
    if inline_note && found.contains("Status note") {
        return Err("duplicate Retry Handoff status note".into());
    }
    if !found.contains("Problem") || !found.contains("Next human step") || !found.contains("Status")
    {
        return Err("Retry Handoff is missing required fields".into());
    }
    if !changed {
        return Err("Retry Handoff already has canonical fields".into());
    }
    Ok(lines.concat().into_bytes())
}

/// Line indices inside one `## <name>` section, excluding fenced regions.
fn unfenced_section_indices(lines: &[String], heading: &str) -> Vec<usize> {
    let mut in_section = false;
    let mut fence: Option<&str> = None;
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let content = line_content(line);
        let trimmed = content.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = &trimmed[..3];
            if fence.is_some_and(|active| active == marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        if content.starts_with("## ") {
            in_section = content == heading;
            continue;
        }
        if in_section {
            out.push(index);
        }
    }
    out
}

/// A task-looking bullet the bullet-shape repair recognizes. Spacing around
/// the dash, checkbox and optional bold wrapper is presentation. The ID's
/// numeric value, checkbox state, wrapper and everything after the ID are
/// protected.
struct TaskBulletShape<'a> {
    indent: &'a str,
    checkbox: &'a str,
    bold: bool,
    digits: &'a str,
    rest: &'a str,
}

impl TaskBulletShape<'_> {
    fn number(&self) -> Result<u32, String> {
        self.digits
            .parse()
            .map_err(|_| "invalid task ID digits".to_string())
    }

    fn canonical(&self) -> Result<String, String> {
        let wrap = if self.bold { "**" } else { "" };
        Ok(format!(
            "{}- {} {wrap}T-{:02}{wrap}{}",
            self.indent,
            self.checkbox,
            self.number()?,
            self.rest
        ))
    }
}

fn task_bullet_shape(content: &str) -> Result<Option<TaskBulletShape<'_>>, String> {
    let indent_len = content.len() - content.trim_start_matches(' ').len();
    let (indent, after_indent) = content.split_at(indent_len);
    let Some(after_dash) = after_indent.strip_prefix('-') else {
        return Ok(None);
    };
    let after_dash = after_dash.trim_start_matches(' ');
    let checkbox = if after_dash.starts_with("[ ]") {
        "[ ]"
    } else if after_dash.starts_with("[x]") {
        "[x]"
    } else {
        return Ok(None);
    };
    let tail = after_dash[3..].trim_start_matches(' ');
    let (bold, body) = match tail.strip_prefix("**") {
        Some(body) => (true, body),
        None => (false, tail),
    };
    let Some(number) = body.strip_prefix("T-") else {
        return Ok(None);
    };
    let digit_len = number.bytes().take_while(u8::is_ascii_digit).count();
    if digit_len == 0 {
        return Ok(None);
    }
    let digits = &number[..digit_len];
    let mut rest = &number[digit_len..];
    if bold {
        rest = rest
            .strip_prefix("**")
            .ok_or("unmatched bold task wrapper")?;
    } else if rest.starts_with("**") {
        return Err("unmatched bold task wrapper".into());
    }
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err("task ID has a word suffix".into());
    }
    if digits.len() > 2 {
        return Err("unsupported task ID width".into());
    }
    Ok(Some(TaskBulletShape {
        indent,
        checkbox,
        bold,
        digits,
        rest,
    }))
}

/// Boundary-delimited `T-<digits>` tokens as `(start, end, digits)`.
fn task_tokens(text: &str) -> Vec<(usize, usize, &str)> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("T-") {
        let start = from + offset;
        from = start + 2;
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        let digits = text[start + 2..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        let end = start + 2 + digits;
        if digits == 0
            || bytes
                .get(end)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            continue;
        }
        tokens.push((start, end, &text[start + 2..end]));
        from = end;
    }
    tokens
}

fn rewrite_task_references(
    cell: &str,
    defined: &std::collections::BTreeSet<u32>,
) -> Result<String, String> {
    let mut out = String::with_capacity(cell.len() + 2);
    let mut last = 0;
    for (start, end, digits) in task_tokens(cell) {
        if digits.len() > 2 {
            return Err("unsupported task reference width".into());
        }
        let number: u32 = digits
            .parse()
            .map_err(|_| "invalid task reference digits")?;
        if !defined.contains(&number) {
            return Err("unresolved task reference prevents numeric normalization".into());
        }
        out.push_str(&cell[last..start]);
        out.push_str(&format!("T-{number:02}"));
        last = end;
    }
    out.push_str(&cell[last..]);
    Ok(out)
}

fn bullet_shape_candidate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut lines = lines_with_endings(bytes)?;
    let mut defined = std::collections::BTreeSet::new();
    let mut padded = false;
    let mut edits = Vec::new();
    for index in unfenced_section_indices(&lines, "## Tasks") {
        let content = line_content(&lines[index]);
        let Some(shape) = task_bullet_shape(content)? else {
            continue;
        };
        if !defined.insert(shape.number()?) {
            return Err("duplicate task IDs prevent numeric normalization".into());
        }
        padded |= shape.digits.len() == 1;
        let canonical = shape.canonical()?;
        if canonical != content {
            edits.push((index, canonical));
        }
    }
    if edits.is_empty() {
        return Err("no supported task bullet shape found".into());
    }
    for (index, replacement) in edits {
        let ending = lines[index][line_content(&lines[index]).len()..].to_string();
        lines[index] = format!("{replacement}{ending}");
    }

    let mut covers_column = None;
    for index in unfenced_section_indices(&lines, "## Test Plan") {
        let line = &lines[index];
        let content = line_content(line).to_string();
        let Some(cells) = table_cells(&content) else {
            continue;
        };
        let normalized: Vec<String> = cells
            .iter()
            .map(|cell| cell.trim().to_ascii_lowercase())
            .collect();
        if normalized.iter().any(|cell| cell == "id")
            && normalized.iter().any(|cell| cell == "covers")
        {
            covers_column = normalized.iter().position(|cell| cell == "covers");
            continue;
        }
        let Some(column) = covers_column else {
            continue;
        };
        let Some(covers) = cells.get(column) else {
            return Err("Test Plan row has no recognized Covers cell".into());
        };
        let rewritten = rewrite_task_references(covers, &defined)?;
        if rewritten != *covers {
            let mut cells = cells.into_iter().map(str::to_owned).collect::<Vec<_>>();
            cells[column] = rewritten;
            let leading = content.len() - content.trim_start().len();
            let trailing = content.len() - content.trim_end().len();
            let newline = line[content.len()..].to_string();
            lines[index] = format!(
                "{}|{}|{}{}",
                &content[..leading],
                cells.join("|"),
                &content[content.len() - trailing..],
                newline
            );
        }
    }
    let repaired = lines.concat();
    if padded
        && task_tokens(&repaired)
            .iter()
            .any(|(_, _, digits)| digits.len() == 1)
    {
        return Err(
            "one-digit task reference outside recognized fields prevents numeric normalization"
                .into(),
        );
    }
    Ok(repaired.into_bytes())
}

fn candidate(id: &RepairId, bytes: &[u8]) -> Result<Vec<u8>, String> {
    match id {
        RepairId::LeadingBom if bytes.starts_with(&[0xef, 0xbb, 0xbf]) => Ok(bytes[3..].to_vec()),
        RepairId::HeadingLevel => {
            let text = std::str::from_utf8(bytes).map_err(|_| "target is not UTF-8")?;
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            let matches: Vec<_> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.trim_end_matches(['\r', '\n']) == "# Tasks")
                .collect();
            if matches.len() != 1 {
                return Err("heading repair requires one exact '# Tasks' line".into());
            }
            let (index, line) = matches[0];
            let replacement = line.replacen("# Tasks", "## Tasks", 1);
            let mut result = Vec::with_capacity(bytes.len() + 1);
            for (line_index, original) in lines.iter().enumerate() {
                result.extend_from_slice(if line_index == index {
                    replacement.as_bytes()
                } else {
                    original.as_bytes()
                });
            }
            Ok(result)
        }
        RepairId::BulletShape => bullet_shape_candidate(bytes),
        RepairId::TableShape => table_shape_candidate(bytes),
        RepairId::RetryFieldShape => retry_field_candidate(bytes),
        _ => Err("repair ID does not match the captured defect".into()),
    }
}

/// Replace each boundary-delimited task token with its numeric identity.
fn normalize_task_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end, digits) in task_tokens(text) {
        out.push_str(&text[last..start]);
        match digits.parse::<u32>() {
            Ok(number) => out.push_str(&format!("<T#{number}>")),
            Err(_) => out.push_str(&text[start..end]),
        }
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

/// Semantic record for bullet-shape: task bullets keep indent, checkbox,
/// wrapper, numeric identity and their exact remainder. Test Plan rows keep
/// every byte except the width of task-ID tokens in the recognized Covers
/// column, the only cell the candidate rewrites. All other lines are exact.
fn bullet_projection(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let lines = lines_with_endings(bytes)?;
    let tasks: std::collections::BTreeSet<usize> = unfenced_section_indices(&lines, "## Tasks")
        .into_iter()
        .collect();
    let plan: std::collections::BTreeSet<usize> = unfenced_section_indices(&lines, "## Test Plan")
        .into_iter()
        .collect();
    let mut covers_column = None;
    let mut out = String::with_capacity(bytes.len());
    for (index, line) in lines.iter().enumerate() {
        let content = line_content(line);
        let newline = &line[content.len()..];
        if tasks.contains(&index) {
            if let Ok(Some(shape)) = task_bullet_shape(content) {
                if let Ok(number) = shape.number() {
                    out.push_str(&format!(
                        "<TASK indent={} checkbox={} bold={} id={number}>{}{newline}",
                        shape.indent.len(),
                        shape.checkbox,
                        shape.bold,
                        shape.rest
                    ));
                    continue;
                }
            }
        } else if plan.contains(&index) {
            if let Some(cells) = table_cells(content) {
                let normalized: Vec<String> = cells
                    .iter()
                    .map(|cell| cell.trim().to_ascii_lowercase())
                    .collect();
                if normalized.iter().any(|cell| cell == "id")
                    && normalized.iter().any(|cell| cell == "covers")
                {
                    covers_column = normalized.iter().position(|cell| cell == "covers");
                } else if let Some(column) = covers_column.filter(|column| *column < cells.len()) {
                    let mut cells: Vec<String> = cells.into_iter().map(str::to_owned).collect();
                    cells[column] = normalize_task_tokens(&cells[column]);
                    let lead = &content[..content.len() - content.trim_start().len()];
                    let trail = &content[content.trim_end().len()..];
                    out.push_str(&format!("<ROW>{lead}|{}|{trail}{newline}", cells.join("|")));
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    Ok(out.into_bytes())
}

/// Semantic record for table-shape: a delimiter row keeps only its arity and
/// per-cell alignment. All other lines are exact.
fn table_projection(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let lines = lines_with_endings(bytes)?;
    let mut out = String::with_capacity(bytes.len());
    for line in &lines {
        let content = line_content(line);
        let signature = table_cells(content).and_then(|cells| {
            let mut signature = String::from("<DELIMITER");
            for cell in cells {
                let raw = cell.trim();
                let core = raw.trim_matches(':');
                if !core.contains('-') || !core.bytes().all(|byte| byte == b'-' || byte == b' ') {
                    return None;
                }
                signature.push(' ');
                signature.push(if raw.starts_with(':') { 'L' } else { '-' });
                signature.push(if raw.ends_with(':') { 'R' } else { '-' });
            }
            signature.push('>');
            Some(signature)
        });
        match signature {
            Some(signature) => {
                out.push_str(&signature);
                out.push_str(&line[content.len()..]);
            }
            None => out.push_str(line),
        }
    }
    Ok(out.into_bytes())
}

/// Semantic record for retry-field-shape: inside the selected Retry Handoff
/// block, known fields keep their label identity and value, and an inline
/// status note equals a separate `Status note` line with the same suffix.
fn retry_projection(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let lines = lines_with_endings(bytes)?;
    let Ok((start, end)) = retry_block_range(&lines) else {
        return Ok(bytes.to_vec());
    };
    let mut out = String::with_capacity(bytes.len());
    for (index, line) in lines.iter().enumerate() {
        let content = line_content(line);
        let newline = &line[content.len()..];
        if index < start || index >= end {
            out.push_str(line);
            continue;
        }
        match retry_field_label(content) {
            Some(("Status", value)) => {
                let value = value.trim_start();
                match value.split_once(" — ") {
                    Some((status, suffix)) => out.push_str(&format!(
                        "<STATUS>{status}{newline}<STATUS-NOTE> — {suffix}{newline}"
                    )),
                    None => out.push_str(&format!("<STATUS>{value}{newline}")),
                }
            }
            Some(("Status note", value)) => {
                out.push_str(&format!("<STATUS-NOTE>{value}{newline}"));
            }
            Some((label, value)) => out.push_str(&format!(
                "<FIELD {label}>{}{newline}",
                value.strip_prefix(' ').unwrap_or(value)
            )),
            None => out.push_str(line),
        }
    }
    Ok(out.into_bytes())
}

fn protected_projection(id: &RepairId, bytes: &[u8]) -> Result<Vec<u8>, String> {
    match id {
        RepairId::LeadingBom => Ok(bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(bytes)
            .to_vec()),
        RepairId::HeadingLevel => {
            let text = std::str::from_utf8(bytes).map_err(|_| "target is not UTF-8")?;
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            let matching: Vec<_> = lines
                .iter()
                .filter(|line| {
                    let content = line.trim_end_matches(['\r', '\n']);
                    content == "# Tasks" || content == "## Tasks"
                })
                .collect();
            if matching.len() != 1 {
                return Err("protected-record comparison requires one Tasks heading".into());
            }
            let mut projection = Vec::with_capacity(bytes.len());
            for line in lines {
                let content = line.trim_end_matches(['\r', '\n']);
                if content == "# Tasks" || content == "## Tasks" {
                    projection.extend_from_slice(b"<TASKS-HEADING>");
                    projection.extend_from_slice(&line[content.len()..].as_bytes());
                } else {
                    projection.extend_from_slice(line.as_bytes());
                }
            }
            Ok(projection)
        }
        RepairId::BulletShape => bullet_projection(bytes),
        RepairId::TableShape => table_projection(bytes),
        RepairId::RetryFieldShape => retry_projection(bytes),
    }
}

fn ensure_protected_records_unchanged(
    id: &RepairId,
    original: &[u8],
    candidate: &[u8],
) -> Result<(), String> {
    if protected_projection(id, original)? != protected_projection(id, candidate)? {
        Err("repair candidate changes protected records".into())
    } else {
        Ok(())
    }
}

/// Which bound target a supported check reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// A source plan: `.md` that is not an execution prompt.
    Source,
    /// An execution prompt: `.prompt.md`.
    Prompt,
}

impl TargetKind {
    fn matches(self, path: &str) -> bool {
        match self {
            TargetKind::Prompt => path.ends_with(".prompt.md"),
            TargetKind::Source => path.ends_with(".md") && !path.ends_with(".prompt.md"),
        }
    }
}

/// One supported `(repair, gate, mode, check)` pairing. The receipt
/// classifier and the helper read this single table, so a nomination and
/// its acceptance cannot drift apart.
struct RepairSupport {
    repair: &'static str,
    gate: &'static str,
    mode: Option<&'static str>,
    check: &'static str,
    target: TargetKind,
}

/// Gate name every pipeline-preflight receipt binds in its repair context.
pub(crate) const PREFLIGHT_GATE: &str = "preflight";
/// The only preflight mode eligible for catalog repair. Terminal-reverify
/// stays read-only, so its receipts never nominate a repair.
pub(crate) const PREFLIGHT_ORDINARY_MODE: &str = "ordinary";

/// Closed nomination table. A failed check outside it stays `owner`. The
/// boundary, converge, handback and quickfix gates are absent on purpose:
/// their failures are not single-target presentation defects, or their
/// validator reads surfaces the single-target probe does not hold.
const REPAIR_SUPPORT: &[RepairSupport] = &[
    RepairSupport {
        repair: "leading-bom",
        gate: "planning",
        mode: None,
        check: "required-sections",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "leading-bom",
        gate: "prompt",
        mode: None,
        check: "tasks-heading",
        target: TargetKind::Prompt,
    },
    RepairSupport {
        repair: "heading-level",
        gate: "prompt",
        mode: None,
        check: "tasks-heading",
        target: TargetKind::Prompt,
    },
    RepairSupport {
        repair: "bullet-shape",
        gate: "planning",
        mode: None,
        check: "machine-anchor-parity",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "bullet-shape",
        gate: "refining",
        mode: None,
        check: "id-well-formed",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "bullet-shape",
        gate: "prompt",
        mode: None,
        check: "unchecked-task-anchor",
        target: TargetKind::Prompt,
    },
    RepairSupport {
        repair: "bullet-shape",
        gate: PREFLIGHT_GATE,
        mode: Some(PREFLIGHT_ORDINARY_MODE),
        check: "tasks-well-formed",
        target: TargetKind::Prompt,
    },
    RepairSupport {
        repair: "table-shape",
        gate: "refining",
        mode: None,
        check: "test-plan-coverage",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "table-shape",
        gate: "refining",
        mode: None,
        check: "owner-acceptance",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "table-shape",
        gate: "refining",
        mode: None,
        check: "preconditions",
        target: TargetKind::Source,
    },
    RepairSupport {
        repair: "table-shape",
        gate: "finalize",
        mode: Some("hygiene-only"),
        check: "finalize-review-shape",
        target: TargetKind::Prompt,
    },
    RepairSupport {
        repair: "retry-field-shape",
        gate: PREFLIGHT_GATE,
        mode: Some(PREFLIGHT_ORDINARY_MODE),
        check: "retry-handoff-shape",
        target: TargetKind::Prompt,
    },
];

fn supported<'a>(
    gate: &'a str,
    mode: &'a str,
) -> impl Iterator<Item = &'static RepairSupport> + 'a {
    REPAIR_SUPPORT
        .iter()
        .filter(move |support| support.gate == gate && support.mode.is_none_or(|m| m == mode))
}

/// Rerun the failed check's existing pure validator on candidate bytes.
fn check_passes(gate: &str, check: &str, bytes: &[u8], repo_root: &Path) -> bool {
    use super::finalize_check::CheckState;
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let outcome = match (gate, check) {
        ("planning", "required-sections") => super::planning_check::required_sections_outcome(text),
        ("planning", "machine-anchor-parity") => {
            super::planning_check::parity_outcome(text, repo_root)
        }
        ("prompt", _) => match super::prompt_check::run_checks(text)
            .into_iter()
            .find(|outcome| outcome.name == check)
        {
            Some(outcome) => outcome,
            None => return false,
        },
        ("refining", "id-well-formed") => super::refining_check::id_well_formed_outcome(text),
        ("refining", "test-plan-coverage") => {
            super::refining_check::test_plan_coverage_for_plan(text)
        }
        ("refining", "owner-acceptance") => super::refining_check::owner_acceptance_outcome(text),
        ("refining", "preconditions") => super::refining_check::preconditions_outcome(text),
        (PREFLIGHT_GATE, "tasks-well-formed") => {
            super::pipeline_preflight::check_tasks_well_formed(text)
        }
        (PREFLIGHT_GATE, "retry-handoff-shape") => {
            super::pipeline_preflight::check_retry_handoff_shape(text)
        }
        ("finalize", "finalize-review-shape") => {
            super::finalize_check::evaluate_finalize_review_text(
                text,
                repo_root,
                super::finalize_check::ReviewIndependenceRule::Any,
            )
        }
        _ => return false,
    };
    outcome.state == CheckState::Pass
}

/// The eligibility probe shared by nomination and the helper. It builds the
/// in-memory candidate, proves protected-record equality and idempotence, and
/// requires the failed check's own validator to pass on the candidate.
fn eligible_candidate(
    support: &RepairSupport,
    path: &str,
    bytes: &[u8],
    repo_root: &Path,
) -> Result<Vec<u8>, String> {
    if !support.target.matches(path) {
        return Err("repair target kind does not match the failed check".into());
    }
    let id = RepairId::parse(support.repair)?;
    let repaired = candidate(&id, bytes)?;
    if repaired == bytes {
        return Err("repair candidate did not change bytes".into());
    }
    ensure_protected_records_unchanged(&id, bytes, &repaired)?;
    if candidate(&id, &repaired).is_ok() {
        return Err("repair candidate is not idempotent".into());
    }
    if !check_passes(support.gate, support.check, &repaired, repo_root) {
        return Err("candidate does not pass the failed check's validator".into());
    }
    Ok(repaired)
}

/// Nominate the first catalogued repair whose eligibility probe succeeds on
/// these exact target bytes for this failed check. `None` means `owner`.
pub(crate) fn nominate(
    gate: &str,
    mode: &str,
    check: &str,
    path: &str,
    bytes: &[u8],
    repo_root: &Path,
) -> Option<&'static str> {
    supported(gate, mode)
        .filter(|support| support.check == check)
        .find(|support| eligible_candidate(support, path, bytes, repo_root).is_ok())
        .map(|support| support.repair)
}

/// The exact disposition line a receipt renders for a nominated repair.
pub(crate) fn format_disposition_line(check: &str, repair: &str, target: &str) -> String {
    format!("- {check}: format (repair_id: {repair}, target: {target})")
}

fn parse_args(
    args: &[String],
) -> Result<(PathBuf, PathBuf, RepairId, String, String, String, u8), String> {
    let mut target = None;
    let mut receipt = None;
    let mut id = None;
    let mut gate = None;
    let mut mode = None;
    let mut attempt = None;
    let mut round = None;
    let mut index = 1;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or("missing format-repair option value")?;
        let slot = match args[index].as_str() {
            "--target" => &mut target,
            "--receipt" => &mut receipt,
            "--repair-id" => &mut id,
            "--gate" => &mut gate,
            "--mode" => &mut mode,
            "--attempt" => &mut attempt,
            "--round" => &mut round,
            option => return Err(format!("unknown format-repair option '{option}'")),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(format!("duplicate format-repair option '{}'", args[index]));
        }
        index += 2;
    }
    let round = round
        .ok_or("missing --round")?
        .parse::<u8>()
        .map_err(|_| "invalid --round")?;
    Ok((
        target.ok_or("missing --target")?.into(),
        receipt.ok_or("missing --receipt")?.into(),
        RepairId::parse(&id.ok_or("missing --repair-id")?)?,
        gate.ok_or("missing --gate")?,
        mode.ok_or("missing --mode")?,
        attempt.ok_or("missing --attempt")?,
        round,
    ))
}

/// Require the receipt's bound repair context to name exactly this gate,
/// mode, gate attempt and repair round.
fn ensure_receipt_context(
    receipt: &str,
    gate: &str,
    mode: &str,
    attempt: &str,
    round: u8,
) -> Result<(), String> {
    let round_text = round.to_string();
    for (field, expected) in [
        ("gate", gate),
        ("mode", mode),
        ("gate_attempt_id", attempt),
        ("repair_round", round_text.as_str()),
    ] {
        if !receipt
            .lines()
            .any(|line| line == format!("{field}: {expected}"))
        {
            return Err(format!("receipt repair context does not match {field}"));
        }
    }
    Ok(())
}

/// Accept only a nomination the receipt declares for this exact target, then
/// rerun the same eligibility probe the receipt classifier ran on the bytes
/// the receipt bound. Every declared check must pass on the candidate.
fn authorize_nomination(
    receipt: &str,
    id: &RepairId,
    gate: &str,
    mode: &str,
    target: &str,
    original: &[u8],
    repo_root: &Path,
) -> Result<(Vec<&'static RepairSupport>, Vec<u8>), String> {
    if receipt_has_owner_row(receipt) {
        return Err("receipt has an owner row, so the whole repair set is refused".into());
    }
    let declared = declared_supports(receipt, id, gate, mode, target);
    let Some(first) = declared.first() else {
        return Err("receipt does not declare this format repair".into());
    };
    let candidate = eligible_candidate(first, target, original, repo_root)?;
    if !declared
        .iter()
        .all(|support| check_passes(support.gate, support.check, &candidate, repo_root))
    {
        return Err("candidate does not pass every declared check validator".into());
    }
    Ok((declared, candidate))
}

/// Whether the receipt declares any `owner` disposition row. One `owner` row
/// refuses every `format` row in the same receipt.
fn receipt_has_owner_row(receipt: &str) -> bool {
    receipt
        .lines()
        .any(|line| line.starts_with("- ") && line.ends_with(": owner (repair_id: none)"))
}

/// The helper's receipt-context and nomination acceptance without the
/// executable envelope or publication, for probes in other gate modules.
#[cfg(test)]
pub(crate) fn helper_accepts(
    receipt: &str,
    repair: &str,
    gate: &str,
    mode: &str,
    attempt: &str,
    target: &str,
    original: &[u8],
    repo_root: &Path,
) -> Result<Vec<u8>, String> {
    ensure_receipt_context(receipt, gate, mode, attempt, 0)?;
    let id = RepairId::parse(repair)?;
    authorize_nomination(receipt, &id, gate, mode, target, original, repo_root)
        .map(|(_, candidate)| candidate)
}

fn execute(args: &[String]) -> Result<(), String> {
    let (target, receipt_path, id, gate, mode, attempt, round) = parse_args(args)?;
    let receipt =
        std::fs::read_to_string(&receipt_path).map_err(|e| format!("cannot read receipt: {e}"))?;
    ensure_receipt_context(&receipt, &gate, &mode, &attempt, round)?;
    let envelope = parse_receipt_envelope(&receipt)?;
    let expected = receipt_envelope(
        Some(&envelope.binding_scope),
        envelope.plan_scope.as_deref().map(|_| target.as_path()),
    )?;
    super::finalize_check::verify_receipt_envelope(&receipt, &expected)?;
    let root = super::pipeline_execution::current_worktree_root()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let target = canonical_inside(&root, &target)?;
    let target_relative = relative_text(&root, &target)?;
    let targets = parse_receipt_targets(&receipt, &root)?;
    let bound = targets
        .iter()
        .find(|(relative, _, _)| relative == &target_relative)
        .ok_or("receipt does not bind the requested target")?;
    let original = bound.2.clone();
    let context = RepairContext {
        gate: gate.clone(),
        mode: mode.clone(),
        attempt_id: attempt.clone(),
        round,
        active_run_id: None,
        targets: targets
            .iter()
            .map(|(path, hash, bytes)| RepairTarget {
                path: path.clone(),
                sha256: hash.clone(),
                raw_bytes: bytes.clone(),
            })
            .collect(),
        factual_inputs: Vec::new(),
    };
    validate_repair_context(&context, &gate, &mode)?;
    let repair_name = id.name();
    let (declared, mut candidate) = authorize_nomination(
        &receipt,
        &id,
        &gate,
        &mode,
        &target_relative,
        &original,
        &root,
    )?;
    let first = declared[0];
    if let Some(meta) = super::planning_authority::parse_localized_meta(
        std::str::from_utf8(&original).map_err(|_| "planning source is not UTF-8")?,
    ) {
        let prior_source =
            recover_planning_record(&root, &target_relative, &meta.rendered_source_hash)?;
        if protected_projection(&id, prior_source.as_bytes())?
            != protected_projection(&id, &candidate)?
        {
            return Err(
                "planning repair changes the historically accepted protected record".into(),
            );
        }
        candidate = append_deviation_audit_row(&candidate, repair_name)?.into_bytes();
        if meta.prompt_hash == super::planning_authority::PENDING_PROMPT_HASH
            && meta.equivalence_verdict == super::planning_authority::PENDING_EQUIVALENCE_VERDICT
        {
            let draft_relative = Path::new(&meta.semantic_draft);
            let draft_relative = draft_relative
                .to_str()
                .filter(|_| !draft_relative.is_absolute())
                .ok_or("semantic draft path is not repository-relative")?;
            let draft = recover_planning_record(&root, draft_relative, &meta.draft_hash)?;
            if let Ok(current_draft) = std::fs::read_to_string(root.join(draft_relative)) {
                if super::planning_authority::rendered_source_hash(&current_draft)
                    == meta.draft_hash
                    && super::planning_authority::semantic_body_record(&current_draft)
                        != super::planning_authority::semantic_body_record(&draft)
                {
                    return Err("current and historical semantic drafts disagree".into());
                }
            }
            let stamped = super::planning_authority::stamp_metadata_bytes(
                std::str::from_utf8(&candidate).map_err(|_| "planning source is not UTF-8")?,
                &draft,
            )
            .map_err(|error| error.to_string())?;
            super::planning_authority::verify_stamp_freshness(&stamped, None)
                .map_err(|error| error.to_string())?;
            candidate = stamped.into_bytes();
        } else if meta.equivalence_verdict == "EQUIVALENT"
            && meta.prompt_hash != super::planning_authority::PENDING_PROMPT_HASH
        {
            let prompt_relative = target_relative
                .strip_suffix(".md")
                .map(|stem| format!("{stem}.prompt.md"))
                .ok_or("cannot derive planning prompt path")?;
            let prompt = recover_planning_record(&root, &prompt_relative, &meta.prompt_hash)?;
            let stamped = super::planning_authority::refresh_equivalence_bytes(
                std::str::from_utf8(&candidate).map_err(|_| "planning source is not UTF-8")?,
                &prompt,
            )
            .map_err(|error| error.to_string())?;
            super::planning_authority::verify_stamp_freshness(&stamped, Some(&prompt))
                .map_err(|error| error.to_string())?;
            candidate = stamped.into_bytes();
        } else {
            return Err("planning stamp requires a preserved pending or EQUIVALENT verdict".into());
        }
    } else {
        candidate = append_repair_deviation_row(&candidate, repair_name, first.check)?;
    }
    // Every declared check must still pass on the final published bytes.
    if !declared
        .iter()
        .all(|support| check_passes(support.gate, support.check, &candidate, &root))
    {
        return Err("candidate does not pass its existing check validator".into());
    }
    let mut leases = acquire_leases(&root, &targets, &attempt)?;
    let operation = (|| {
        for (relative, expected_hash, expected_bytes) in &targets {
            let path = root.join(relative);
            let actual = std::fs::read(&path).map_err(|e| e.to_string())?;
            let hash = format!("{:x}", sha2::Sha256::digest(&actual));
            if hash != *expected_hash || actual != *expected_bytes {
                return Err(format!(
                    "target became stale before publication: {relative}"
                ));
            }
        }
        ensure_clean_target_and_index(&root, &target_relative)?;
        publish_and_commit(
            &root,
            &target,
            &target_relative,
            &original,
            &candidate,
            &gate,
            &leases,
        )
    })();
    let cleanup = release_leases(&mut leases);
    match (operation, cleanup) {
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!(
            "repair committed, but lease release failed: {error}"
        )),
        (Err(error), Err(cleanup)) => Err(format!("{error}; lease cleanup failed: {cleanup}")),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn canonical_inside(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize repair path: {e}"))?;
    if canonical.strip_prefix(root).is_err() {
        return Err("repair target is outside the worktree".into());
    }
    Ok(canonical)
}

fn relative_text(root: &Path, path: &Path) -> Result<String, String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|_| "repair target is outside the worktree")?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn parse_receipt_targets(
    receipt: &str,
    root: &Path,
) -> Result<Vec<(String, String, Vec<u8>)>, String> {
    let mut targets = Vec::new();
    for line in receipt
        .lines()
        .filter(|line| line.starts_with("repair_target: "))
    {
        let value = line.strip_prefix("repair_target: ").unwrap();
        let (relative, hash) = value
            .split_once(" sha256:")
            .ok_or("malformed repair target binding")?;
        let rel = Path::new(relative);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("repair target path is not canonical and repository-relative".into());
        }
        let path = canonical_inside(root, &root.join(rel))?;
        if relative_text(root, &path)? != relative {
            return Err("repair target path is not canonical".into());
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("cannot read repair target '{relative}': {e}"))?;
        let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
        if actual != hash {
            return Err(format!("receipt target bytes are stale: {relative}"));
        }
        if targets.iter().any(|(existing, _, _)| existing == relative) {
            return Err("duplicate repair target binding".into());
        }
        targets.push((relative.to_string(), hash.to_string(), bytes));
    }
    if targets.is_empty() {
        return Err("receipt has no repair target bindings".into());
    }
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(targets)
}

/// Supported pairings the receipt actually nominated for this repair, gate,
/// mode and exact target, in table order.
fn declared_supports(
    receipt: &str,
    id: &RepairId,
    gate: &str,
    mode: &str,
    target: &str,
) -> Vec<&'static RepairSupport> {
    supported(gate, mode)
        .filter(|support| support.repair == id.name())
        .filter(|support| {
            let line = format_disposition_line(support.check, support.repair, target);
            receipt.lines().any(|candidate| candidate == line)
        })
        .collect()
}

fn acquire_leases(
    root: &Path,
    targets: &[(String, String, Vec<u8>)],
    token: &str,
) -> Result<Vec<dispatch::dispatch::PromptWriterLease>, String> {
    let mut leases = Vec::new();
    for (relative, _, _) in targets {
        let path = root.join(relative);
        match dispatch::dispatch::acquire_prompt_writer_lease(root, &path, None, token) {
            Ok(lease) => leases.push(lease),
            Err(error) => {
                let cleanup = release_leases(&mut leases);
                return Err(match cleanup {
                    Ok(()) => format!("lease conflict for '{relative}': {error}"),
                    Err(cleanup) => format!(
                        "lease conflict for '{relative}': {error}; prior lease cleanup failed: {cleanup}"
                    ),
                });
            }
        }
    }
    Ok(leases)
}

fn release_leases(leases: &mut [dispatch::dispatch::PromptWriterLease]) -> Result<(), String> {
    let mut failures = Vec::new();
    for lease in leases {
        if let Err(error) = lease.release_confirmed() {
            failures.push(error.to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn git(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .current_dir(root)
        .arg("--literal-pathspecs")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))
}

/// Recover a prior byte version whose planning hash matches the stored value.
/// The bounded history is deliberately fail-closed when no matching record is
/// reachable or matching commits disagree semantically.
fn recover_planning_record(
    root: &Path,
    relative: &str,
    expected_hash: &str,
) -> Result<String, String> {
    let log = git(root, &["log", "-n", "256", "--format=%H", "--", relative])?;
    if !log.status.success() {
        return Err("cannot inspect planning history".into());
    }
    let mut matched: Option<String> = None;
    for commit in String::from_utf8_lossy(&log.stdout).lines() {
        let snapshot = git(root, &["show", &format!("{commit}:{relative}")])?;
        if !snapshot.status.success() {
            continue;
        }
        let body = String::from_utf8(snapshot.stdout)
            .map_err(|_| format!("historical planning record is not UTF-8: {relative}"))?;
        if super::planning_authority::rendered_source_hash(&body) != expected_hash {
            continue;
        }
        let record = super::planning_authority::semantic_body_record(&body);
        if matched.as_ref().is_some_and(|prior| prior != &record) {
            return Err(format!("distinct matching planning history for {relative}"));
        }
        matched = Some(body);
    }
    matched.ok_or_else(|| format!("no matching planning history for {relative}"))
}

fn append_deviation_audit_row(candidate: &[u8], repair_name: &str) -> Result<String, String> {
    let text = std::str::from_utf8(candidate).map_err(|_| "planning source is not UTF-8")?;
    let task_id = text
        .lines()
        .find_map(|line| line.strip_prefix("Current Task: "))
        .filter(|value| {
            value.strip_prefix("T-").is_some_and(|digits| {
                digits.len() == 2 && digits.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .ok_or("planning repair requires a canonical Current Task for its Deviation row")?;
    let row = format!(
        "| {task_id} | Formatting-only repair | Applied {repair_name} after proving the prior accepted record | Historical protected records matched |\n"
    );
    insert_deviation_row(text, &row)
}

/// Append the deterministic Deviation audit row to a target that carries the
/// canonical `### Deviations` table. A target without that heading has no
/// Deviation surface, so the helper's own commit is its audit record. A
/// present but malformed table refuses the repair.
fn append_repair_deviation_row(
    candidate: &[u8],
    repair_name: &str,
    check: &str,
) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(candidate).map_err(|_| "repair target is not UTF-8")?;
    if !text.lines().any(|line| line.trim_end() == "### Deviations") {
        return Ok(candidate.to_vec());
    }
    let step = match super::converge_check::current_task_value(text) {
        super::converge_check::CurrentTaskValue::Active(task)
            if pipeline::task_spec::parse_plan_task_bullet(&format!("- [ ] {task}"))
                .is_some_and(|bullet| bullet.id == task) =>
        {
            task
        }
        _ => "—".to_string(),
    };
    let row = format!(
        "| {step} | Formatting-only repair | Applied {repair_name} for {check} | Protected records matched and the {check} validator passed |\n"
    );
    insert_deviation_row(text, &row).map(String::into_bytes)
}

fn insert_deviation_row(text: &str, row: &str) -> Result<String, String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let heading = lines
        .iter()
        .position(|line| line.trim_end_matches(['\r', '\n']) == "### Deviations")
        .ok_or("planning repair requires the Deviation table")?;
    let header = (heading + 1..lines.len())
        .find(|index| lines[*index].contains("| Step | Plan Said | Actually Did | Why |"))
        .ok_or("planning repair Deviation table has no canonical header")?;
    let separator = header + 1;
    if separator >= lines.len() || !lines[separator].trim_start().starts_with("| ---") {
        return Err("planning repair Deviation table has no separator".into());
    }
    let mut output = String::with_capacity(text.len() + row.len());
    for (index, line) in lines.iter().enumerate() {
        output.push_str(line);
        if index == separator {
            output.push_str(row);
        }
    }
    Ok(output)
}

fn ensure_clean_target_and_index(root: &Path, target: &str) -> Result<(), String> {
    ensure_index_clean(root)?;
    let diff = git(root, &["diff", "--quiet", "--", target])?;
    if diff.status.code() == Some(1) {
        return Err(format!("dirty repair target refuses publication: {target}"));
    }
    if !diff.status.success() {
        return Err("cannot verify repair target worktree status".into());
    }
    let untracked = git(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", target],
    )?;
    if !untracked.status.success() {
        return Err("cannot verify repair target tracking status".into());
    }
    if !untracked.stdout.is_empty() {
        return Err(format!(
            "untracked repair target refuses publication: {target}"
        ));
    }
    Ok(())
}

fn ensure_index_clean(root: &Path) -> Result<(), String> {
    let output = git(root, &["diff", "--cached", "--quiet"])?;
    if output.status.success() {
        Ok(())
    } else if output.status.code() == Some(1) {
        Err("preexisting staged entries refuse format repair".into())
    } else {
        Err("cannot verify staged index before repair publication".into())
    }
}

fn publish_and_commit(
    root: &Path,
    target: &Path,
    relative: &str,
    original: &[u8],
    candidate: &[u8],
    gate: &str,
    leases: &[dispatch::dispatch::PromptWriterLease],
) -> Result<(), String> {
    publish_and_commit_with(
        root,
        target,
        relative,
        original,
        candidate,
        gate,
        leases,
        || Ok(()),
    )
}

fn publish_and_commit_with<F>(
    root: &Path,
    target: &Path,
    relative: &str,
    original: &[u8],
    candidate: &[u8],
    gate: &str,
    _leases: &[dispatch::dispatch::PromptWriterLease],
    before_commit: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    if std::fs::read(target).ok().as_deref() != Some(original) {
        return Err(format!(
            "target became stale immediately before publication: {relative}"
        ));
    }
    if let Err(error) = std::fs::write(target, candidate) {
        let mut residual = Vec::new();
        if std::fs::read(target).ok().as_deref() == Some(candidate) {
            if let Err(restore_error) = std::fs::write(target, original) {
                residual.push(format!("{} ({restore_error})", target.display()));
            }
        } else if std::fs::read(target).ok().as_deref() != Some(original) {
            residual.push(target.display().to_string());
        }
        if residual.is_empty() && std::fs::read(target).ok().as_deref() == Some(original) {
            return Err(format!(
                "cannot publish repair candidate: {error}; original bytes restored"
            ));
        }
        return Err(format!(
            "cannot publish repair candidate: {error}; restoration unverified, residual paths: {}",
            residual.join(", ")
        ));
    }
    let result = (|| {
        ensure_index_clean(root)?;
        let add = git(root, &["add", "--", relative])?;
        if !add.status.success() {
            return Err(format!(
                "git add failed: {}",
                String::from_utf8_lossy(&add.stderr).trim()
            ));
        }
        ensure_exact_paths(root, &[relative], "staged")?;
        ensure_exact_paths(root, &[relative], "staged before commit")?;
        let tree = git(root, &["write-tree"])?;
        if !tree.status.success() {
            return Err(format!(
                "cannot write verified repair tree: {}",
                String::from_utf8_lossy(&tree.stderr).trim()
            ));
        }
        let previous_head = git(root, &["rev-parse", "HEAD"])?;
        if !previous_head.status.success() {
            return Err("cannot record HEAD before repair commit".into());
        }
        let previous_head = String::from_utf8_lossy(&previous_head.stdout)
            .trim()
            .to_string();
        let tree = String::from_utf8_lossy(&tree.stdout).trim().to_string();
        let commit = git(
            root,
            &[
                "commit-tree",
                &tree,
                "-p",
                &previous_head,
                "-m",
                &format!("docs: repair {gate} formatting"),
            ],
        )?;
        if !commit.status.success() {
            return Err(format!(
                "cannot create repair commit object: {}",
                String::from_utf8_lossy(&commit.stderr).trim()
            ));
        }
        let new_commit = String::from_utf8_lossy(&commit.stdout).trim().to_string();
        let committed = git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "-z",
                &previous_head,
                &new_commit,
            ],
        )?;
        if !committed.status.success() {
            return Err("cannot verify committed repair paths".into());
        }
        let paths = nul_paths(&committed.stdout)?;
        if paths != [relative] {
            return Err(format!(
                "commit object {new_commit} path set differs from declared repair path: {paths:?}"
            ));
        }
        before_commit()?;
        ensure_exact_paths(root, &[relative], "staged before branch update")?;
        let update = git(root, &["update-ref", "HEAD", &new_commit, &previous_head])?;
        if !update.status.success() {
            let observed = git(root, &["rev-parse", "HEAD"])
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|| "unavailable".into());
            return Err(format!(
                "repair branch update refused: previous {previous_head}, repair {new_commit}, observed {observed}; {}",
                String::from_utf8_lossy(&update.stderr).trim()
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut residual = Vec::new();
        if std::fs::read(target).ok().as_deref() == Some(candidate) {
            if let Err(e) = std::fs::write(target, original) {
                residual.push(format!("{} ({e})", target.display()));
            }
        } else if std::fs::read(target).ok().as_deref() != Some(original) {
            residual.push(target.display().to_string());
        }
        let staged = git(root, &["show", &format!(":{relative}")]);
        match staged {
            Ok(output) if output.status.success() && output.stdout == candidate => {
                let reset = git(root, &["reset", "--", relative]);
                if !matches!(reset, Ok(output) if output.status.success()) {
                    residual.push(format!("index:{relative}"));
                }
            }
            Ok(output) if !output.status.success() => {}
            _ => residual.push(format!("index:{relative}")),
        }
        if std::fs::read(target).ok().as_deref() != Some(original) {
            residual.push(target.display().to_string());
        }
        if residual.is_empty() {
            Err(format!("{error}; original bytes and index restored"))
        } else {
            Err(format!(
                "{error}; restoration unverified, residual paths: {}",
                residual.join(", ")
            ))
        }
    } else {
        Ok(())
    }
}

fn ensure_exact_paths(root: &Path, expected: &[&str], label: &str) -> Result<(), String> {
    let output = git(root, &["diff", "--cached", "--name-only", "-z"])?;
    if !output.status.success() {
        return Err(format!("cannot verify {label} repair paths"));
    }
    let paths = nul_paths(&output.stdout)?;
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    if paths.iter().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(format!(
            "{label} path set differs from declared repair paths: {paths:?}"
        ));
    }
    Ok(())
}

fn nul_paths(bytes: &[u8]) -> Result<Vec<String>, String> {
    if bytes.last().is_some_and(|byte| *byte != 0) {
        return Err("Git returned a malformed NUL-delimited path list".into());
    }
    let mut paths = bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|_| "repair path is not valid UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort_unstable();
    Ok(paths)
}

pub(crate) fn cmd_format_repair(args: &[String]) -> ExitCode {
    match execute(args) {
        Ok(()) => ExitCode::Success,
        Err(error) => {
            eprintln!("gal format-repair: {error}");
            ExitCode::Error
        }
    }
}

/// Invocation-local payload repair budget carried by the pipeline writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairInvocation {
    pub invocation_id: String,
    pub next_round: u8,
}

impl RepairInvocation {
    pub(crate) fn parse_optional(args: &[String]) -> Result<(Vec<String>, Option<Self>), String> {
        let mut forwarded = Vec::with_capacity(args.len());
        let mut context = None;
        let mut index = 0;
        while index < args.len() {
            if args[index] == "--format-repair-context" {
                if context.is_some() {
                    return Err("duplicate --format-repair-context".into());
                }
                index += 1;
                let raw = args
                    .get(index)
                    .ok_or("--format-repair-context requires JSON")?;
                let value: serde_json::Value = serde_json::from_str(raw)
                    .map_err(|error| format!("invalid format repair context: {error}"))?;
                let object = value
                    .as_object()
                    .ok_or("format repair context must be an object")?;
                if object.len() != 2 {
                    return Err(
                        "format repair context must contain only invocation_id and round".into(),
                    );
                }
                let invocation_id = object
                    .get("invocation_id")
                    .and_then(|v| v.as_str())
                    .filter(|value| !value.is_empty())
                    .ok_or("format repair context requires invocation_id")?;
                let round = object
                    .get("round")
                    .and_then(|v| v.as_u64())
                    .filter(|value| (1..=2).contains(value))
                    .ok_or("format repair context round must be 1 or 2")?;
                context = Some(Self {
                    invocation_id: invocation_id.to_string(),
                    next_round: round as u8,
                });
            } else {
                forwarded.push(args[index].clone());
            }
            index += 1;
        }
        Ok((forwarded, context))
    }

    /// Normalize only a uniquely evidenced derived payload. The caller retains the sealed bytes.
    pub(crate) fn candidate(
        &mut self,
        original: &[u8],
    ) -> Result<(Vec<u8>, Option<String>), String> {
        let (candidate, repair_id) = derived_payload_candidate(original)?;
        let Some(repair_id) = repair_id else {
            return Ok((original.to_vec(), None));
        };
        if self.next_round > 2 {
            return Err("format repair round limit exceeded".into());
        }
        let round = self.next_round;
        self.next_round += 1;
        eprintln!(
            "FORMAT_REPAIR invocation_id={} round={} repair_id={} original_sha256={} derived_sha256={}",
            self.invocation_id,
            round,
            repair_id,
            format!("{:x}", Sha256::digest(original)),
            format!("{:x}", Sha256::digest(&candidate))
        );
        Ok((candidate, Some(repair_id)))
    }
}

/// Purely derive a lossless candidate from immutable receipt or snapshot bytes.
fn derived_payload_candidate(original: &[u8]) -> Result<(Vec<u8>, Option<String>), String> {
    let mut candidate = original.to_vec();
    let mut repair_id = Vec::new();
    if candidate.starts_with(&[0xef, 0xbb, 0xbf]) {
        candidate.drain(..3);
        repair_id.push("leading-bom");
    }
    let text = std::str::from_utf8(&candidate).map_err(|_| "derived payload is not UTF-8")?;
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    fn marker_text(line: &str) -> &str {
        line.trim_end_matches(['\r', '\n']).trim_start()
    }
    let pairs: Vec<_> = lines
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| {
            marker_text(pair[0]).starts_with("Command:")
                && marker_text(pair[1]).starts_with("Result:")
        })
        .map(|(i, _)| i)
        .collect();
    let markers = lines
        .iter()
        .filter(|line| marker_text(line).starts_with("Evidence:"))
        .count();
    let command_or_result = lines.iter().any(|line| {
        let line = marker_text(line);
        line.starts_with("Command:") || line.starts_with("Result:")
    });
    if pairs.len() == 1 && markers == 0 {
        let result_line = lines[pairs[0] + 1];
        let insert_at = lines
            .iter()
            .take(pairs[0] + 2)
            .map(|line| line.len())
            .sum::<usize>();
        let newline = if result_line.ends_with("\r\n") {
            b"\r\n".as_slice()
        } else {
            b"\n".as_slice()
        };
        let needs_separator = !result_line.ends_with('\n');
        let mut inserted = Vec::with_capacity(candidate.len() + 12);
        inserted.extend_from_slice(&candidate[..insert_at]);
        if needs_separator {
            inserted.extend_from_slice(newline);
        }
        inserted.extend_from_slice(b"Evidence:");
        inserted.extend_from_slice(newline);
        inserted.extend_from_slice(&candidate[insert_at..]);
        candidate = inserted;
        repair_id.push("evidence-marker");
    } else if pairs.len() > 1 || markers > 1 || (command_or_result && pairs.is_empty()) {
        return Err("derived payload evidence markers are ambiguous".into());
    }
    if repair_id.is_empty() {
        return Ok((candidate, None));
    }
    let repair_id = repair_id.join("+");
    Ok((candidate, Some(repair_id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn init_recovery_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("primary");
        let linked = temp.path().join("linked");
        fs::create_dir_all(&primary).unwrap();
        fixture_git(&primary, &["init", "-q", "-b", "main"]);
        fixture_git(
            &primary,
            &["config", "user.email", "fixture@example.invalid"],
        );
        fixture_git(&primary, &["config", "user.name", "Fixture"]);
        fixture_git(&primary, &["config", "commit.gpgsign", "false"]);
        fs::create_dir_all(primary.join(".dev/plans")).unwrap();
        fs::write(primary.join(".dev/plans/entry.prompt.md"), "prompt\n").unwrap();
        fs::write(primary.join("conflict.txt"), "base\n").unwrap();
        fs::write(primary.join("unrelated.txt"), "unrelated\n").unwrap();
        fixture_git(&primary, &["add", "--all"]);
        fixture_git(&primary, &["commit", "-qm", "base"]);
        fixture_git(
            &primary,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "linked",
                linked.to_str().unwrap(),
            ],
        );
        (temp, primary, linked)
    }

    #[test]
    fn recovery_merges_primary_entry_prompt_before_binding() {
        let (_temp, primary, linked) = init_recovery_fixture();
        fs::write(primary.join(".dev/plans/entry.prompt.md"), "new prompt\n").unwrap();
        fixture_git(&primary, &["add", "--", ".dev/plans/entry.prompt.md"]);
        fixture_git(&primary, &["commit", "-qm", "publish prompt"]);

        let prompt = linked.join(".dev/plans/entry.prompt.md");
        assert!(recover_primary_tip(&linked, &prompt).unwrap());
        assert_eq!(
            fs::read_to_string(prompt).unwrap().replace("\r\n", "\n"),
            "new prompt\n"
        );
        assert_eq!(
            fixture_git(&linked, &["rev-parse", "HEAD"]),
            fixture_git(&primary, &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn recovery_preserves_prompt_committed_only_on_linked_branch() {
        let (_temp, _primary, linked) = init_recovery_fixture();
        fs::write(
            linked.join(".dev/plans/branch-entry.prompt.md"),
            "branch prompt\n",
        )
        .unwrap();
        fixture_git(&linked, &["add", "--", ".dev/plans/branch-entry.prompt.md"]);
        fixture_git(&linked, &["commit", "-qm", "linked-only prompt"]);
        let before = fixture_git(&linked, &["rev-parse", "HEAD"]);

        assert!(
            !recover_primary_tip(&linked, &linked.join(".dev/plans/branch-entry.prompt.md"))
                .unwrap()
        );
        assert_eq!(fixture_git(&linked, &["rev-parse", "HEAD"]), before);
        assert_eq!(
            fs::read_to_string(linked.join(".dev/plans/branch-entry.prompt.md")).unwrap(),
            "branch prompt\n"
        );
    }

    #[test]
    fn recovery_refuses_detached_primary_without_changing_linked_head() {
        let (_temp, primary, linked) = init_recovery_fixture();
        let before = fixture_git(&linked, &["rev-parse", "HEAD"]);
        let primary_head = fixture_git(&primary, &["rev-parse", "HEAD"]);
        fixture_git(
            &primary,
            &["update-ref", "--no-deref", "HEAD", &primary_head],
        );

        let error =
            recover_primary_tip(&linked, &linked.join(".dev/plans/entry.prompt.md")).unwrap_err();
        assert!(error.contains("detached or has no branch"), "{error}");
        assert_eq!(fixture_git(&linked, &["rev-parse", "HEAD"]), before);
        assert!(fixture_git(&linked, &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn recovery_aborts_conflict_and_restores_linked_state() {
        let (_temp, primary, linked) = init_recovery_fixture();
        fs::write(linked.join("conflict.txt"), "linked\n").unwrap();
        fixture_git(&linked, &["add", "--", "conflict.txt"]);
        fixture_git(&linked, &["commit", "-qm", "linked edit"]);
        fs::write(primary.join("conflict.txt"), "primary\n").unwrap();
        fixture_git(&primary, &["add", "--", "conflict.txt"]);
        fixture_git(&primary, &["commit", "-qm", "primary edit"]);
        let before = fixture_git(&linked, &["rev-parse", "HEAD"]);

        let error =
            recover_primary_tip(&linked, &linked.join(".dev/plans/entry.prompt.md")).unwrap_err();
        assert!(
            error.contains("merge conflicted and was aborted"),
            "{error}"
        );
        assert!(error.contains("conflict.txt"), "{error}");
        assert_eq!(fixture_git(&linked, &["rev-parse", "HEAD"]), before);
        assert!(
            fixture_git(&linked, &["status", "--porcelain", "--untracked-files=all"]).is_empty()
        );
        assert_eq!(
            fs::read_to_string(linked.join("conflict.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "linked\n"
        );
    }

    #[test]
    fn recovery_refuses_when_primary_tip_moves_after_capture() {
        let (_temp, _primary, linked) = init_recovery_fixture();
        let before_head = fixture_git(&linked, &["rev-parse", "HEAD"]);
        let before_index = fixture_git(&linked, &["write-tree"]);
        let before_status =
            fixture_git(&linked, &["status", "--porcelain", "--untracked-files=all"]);
        let advance_primary = |primary: &Path, _ref_name: &str| {
            fs::write(primary.join("primary-moved.txt"), "moved\n").unwrap();
            fixture_git(primary, &["add", "--", "primary-moved.txt"]);
            fixture_git(primary, &["commit", "-qm", "move primary tip"]);
        };
        let hooks = RecoveryTestHooks {
            after_tip_captured: &advance_primary,
            abort_failure: None,
        };

        let error = recover_primary_tip_inner(
            &linked,
            &linked.join(".dev/plans/entry.prompt.md"),
            Some(&hooks),
        )
        .unwrap_err();

        assert!(
            error.contains("primary branch tip moved during recovery"),
            "{error}"
        );
        assert_eq!(fixture_git(&linked, &["rev-parse", "HEAD"]), before_head);
        assert_eq!(fixture_git(&linked, &["write-tree"]), before_index);
        assert_eq!(
            fixture_git(&linked, &["status", "--porcelain", "--untracked-files=all"]),
            before_status
        );
        assert!(!linked.join("primary-moved.txt").exists());
    }

    #[test]
    fn recovery_reports_residual_state_when_abort_fails() {
        let (_temp, primary, linked) = init_recovery_fixture();
        fs::write(linked.join("conflict.txt"), "linked\n").unwrap();
        fixture_git(&linked, &["add", "--", "conflict.txt"]);
        fixture_git(&linked, &["commit", "-qm", "linked edit"]);
        fs::write(primary.join("conflict.txt"), "primary\n").unwrap();
        fixture_git(&primary, &["add", "--", "conflict.txt"]);
        fixture_git(&primary, &["commit", "-qm", "primary edit"]);
        let before_head = fixture_git(&linked, &["rev-parse", "HEAD"]);
        let no_tip_change = |_primary: &Path, _ref_name: &str| {};
        let hooks = RecoveryTestHooks {
            after_tip_captured: &no_tip_change,
            abort_failure: Some("injected abort failure"),
        };

        let error = recover_primary_tip_inner(
            &linked,
            &linked.join(".dev/plans/entry.prompt.md"),
            Some(&hooks),
        )
        .unwrap_err();
        let residual = fixture_git(&linked, &["status", "--porcelain", "--untracked-files=all"]);

        assert!(error.contains("restoration is unproven"), "{error}");
        assert!(error.contains("conflict.txt"), "{error}");
        assert!(error.contains("residual paths:"), "{error}");
        assert!(
            !residual.is_empty(),
            "the unresolved merge must remain visible"
        );
        assert_eq!(fixture_git(&linked, &["rev-parse", "HEAD"]), before_head);
        assert_eq!(
            fs::read_to_string(linked.join("unrelated.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "unrelated\n"
        );
    }

    #[test]
    fn derived_payload_repair_preserves_original_and_requires_unique_evidence_pair() {
        let mut invocation = RepairInvocation {
            invocation_id: "invoke-1".into(),
            next_round: 1,
        };
        let original = b"\xef\xbb\xbfCommand: cargo test\r\nResult: PASS\r\n";
        let (derived, repair_id) = invocation.candidate(original).unwrap();
        assert_eq!(repair_id.as_deref(), Some("leading-bom+evidence-marker"));
        assert_eq!(
            derived,
            b"Command: cargo test\r\nResult: PASS\r\nEvidence:\r\n"
        );
        assert!(original.starts_with(&[0xef, 0xbb, 0xbf]));
        assert_eq!(invocation.next_round, 2);

        let ambiguous = b"Command: one\nResult: PASS\nCommand: two\nResult: PASS\n";
        assert!(invocation.candidate(ambiguous).is_err());
        let evidence_present = b"Command: one\nResult: PASS\nEvidence: supplied\n";
        assert_eq!(invocation.candidate(evidence_present).unwrap().1, None);
        assert!(invocation
            .candidate(b"Command: one\nEvidence: missing result\n")
            .is_err());
    }

    #[test]
    fn repair_context_is_removed_once_and_rejects_duplicate_or_malformed_values() {
        let args = vec![
            "pipeline".into(),
            "prompt.md".into(),
            "--format-repair-context".into(),
            r#"{"invocation_id":"run-1","round":1}"#.into(),
        ];
        let (forwarded, context) = RepairInvocation::parse_optional(&args).unwrap();
        assert_eq!(forwarded, ["pipeline", "prompt.md"]);
        assert_eq!(context.unwrap().next_round, 1);
        let duplicated = [args.clone(), args[2..].to_vec()].concat();
        assert!(RepairInvocation::parse_optional(&duplicated).is_err());
        let malformed = vec![
            "pipeline".into(),
            "--format-repair-context".into(),
            "{}".into(),
        ];
        assert!(RepairInvocation::parse_optional(&malformed).is_err());
    }

    #[test]
    fn candidate_repairs_only_a_unique_known_heading_or_bom() {
        assert_eq!(
            candidate(&RepairId::LeadingBom, b"\xef\xbb\xbf## Tasks\n").unwrap(),
            b"## Tasks\n"
        );
        assert_eq!(
            candidate(&RepairId::HeadingLevel, b"# Tasks\r\nbody\n").unwrap(),
            b"## Tasks\r\nbody\n"
        );
        assert!(candidate(&RepairId::HeadingLevel, b"# Tasks\n# Tasks\n").is_err());
        assert!(candidate(&RepairId::LeadingBom, b"plain\n").is_err());
        let source = b"# Tasks\r\n- [ ] task\r\n";
        let repaired = candidate(&RepairId::HeadingLevel, source).unwrap();
        assert_eq!(
            protected_projection(&RepairId::HeadingLevel, source).unwrap(),
            protected_projection(&RepairId::HeadingLevel, &repaired).unwrap()
        );
    }

    #[test]
    fn added_document_shapes_are_lossless_and_idempotent() {
        let task = format!("T-{}", 1);
        let canonical = format!("T-{:02}", 1);
        let bullet = format!(
            "## Tasks\n-   [ ]   **{task}** — keep  literal  spaces\n\n## Test Plan\n| ID | Description | Covers |\n| :-- | --- | :--- |\n| TP-1 | PASS | {task} |\n"
        );
        let fixed = candidate(&RepairId::BulletShape, bullet.as_bytes()).unwrap();
        let expected = format!(
            "## Tasks\n- [ ] **{canonical}** — keep  literal  spaces\n\n## Test Plan\n| ID | Description | Covers |\n| :-- | --- | :--- |\n| TP-1 | PASS | {canonical} |\n"
        );
        assert_eq!(fixed, expected.as_bytes());
        assert!(candidate(&RepairId::BulletShape, &fixed).is_err());
        let table =
            b"| ID | Description | Covers |\n| :-- | --- | :--- |\n| TP-1 | PASS | task |\n";
        let table_fixed = candidate(&RepairId::TableShape, table).unwrap();
        assert_eq!(
            table_fixed,
            b"| ID | Description | Covers |\n| :--- | --- | :--- |\n| TP-1 | PASS | task |\n"
        );
        assert!(candidate(&RepairId::TableShape, &table_fixed).is_err());
        let retry = format!(
            "#### Retry Handoff — {canonical} / IMPLEMENT\nStatus: OPEN — note stays exact\nProblem: issue\nNext human step: action\n"
        );
        let retry_fixed = candidate(&RepairId::RetryFieldShape, retry.as_bytes()).unwrap();
        assert!(
            String::from_utf8_lossy(&retry_fixed).contains("- Status note: — note stays exact\n")
        );
        assert!(String::from_utf8_lossy(&retry_fixed)
            .contains("- Problem: issue\n- Next human step: action\n"));
        assert!(candidate(&RepairId::RetryFieldShape, &retry_fixed).is_err());
    }

    #[test]
    fn document_shape_refusals_do_not_produce_candidates() {
        let id = format!("T-{}", 1);
        let collision = format!("## Tasks\n- [ ] {id} one\n- [ ] {id} duplicate\n");
        assert!(candidate(&RepairId::BulletShape, collision.as_bytes()).is_err());
        let canonical_collision = format!("## Tasks\n- [ ] {id} one\n- [ ] T-{:02} duplicate\n", 1);
        assert!(candidate(&RepairId::BulletShape, canonical_collision.as_bytes()).is_err());
        let invalid = format!("## Tasks\n- [ ] T-{} invalid\n", 100);
        assert!(candidate(&RepairId::BulletShape, invalid.as_bytes()).is_err());
        let unknown_reference = format!("T-{}", 99);
        let unresolved = format!(
            "## Tasks\n- [ ] {id} task\n\n## Test Plan\n| ID | Description | Covers |\n| --- | --- | --- |\n| TP-1 | PASS | {unknown_reference} |\n"
        );
        assert!(candidate(&RepairId::BulletShape, unresolved.as_bytes()).is_err());
        let inferred =
            b"| ID | Description | Covers |\n| -- | --- | |\n| TP-1 | PASS | inferred |\n";
        assert!(candidate(&RepairId::TableShape, inferred).is_err());
        let covered = format!("T-{:02}", 1);
        let escaped_delimiter = format!(
            "| ID | Description | Covers |\n| \\| | --- | --- |\n| TP-1 | PASS | {covered} |\n"
        );
        assert!(candidate(&RepairId::TableShape, escaped_delimiter.as_bytes()).is_err());
        let id = format!("T-{:02}", 1);
        let unknown = format!(
            "#### Retry Handoff — {id} / IMPLEMENT\nStatus: UNKNOWN — note\nProblem: issue\nNext human step: action\n"
        );
        assert!(candidate(&RepairId::RetryFieldShape, unknown.as_bytes()).is_err());
        let missing =
            format!("#### Retry Handoff — {id} / IMPLEMENT\nStatus: OPEN\nProblem: issue\n");
        assert!(candidate(&RepairId::RetryFieldShape, missing.as_bytes()).is_err());
        assert!(RepairId::parse("index-presentation").is_err());
        for (repair, bytes) in [
            (RepairId::BulletShape, collision.into_bytes()),
            (RepairId::BulletShape, canonical_collision.into_bytes()),
            (RepairId::BulletShape, unresolved.into_bytes()),
            (RepairId::TableShape, inferred.to_vec()),
            (RepairId::RetryFieldShape, unknown.into_bytes()),
        ] {
            let before = bytes.clone();
            assert!(candidate(&repair, &bytes).is_err());
            assert_eq!(bytes, before);
        }
    }

    #[test]
    fn helper_requires_explicit_context_fields_and_rejects_duplicate_flags() {
        let required = vec![
            "format-repair",
            "--target",
            "x",
            "--receipt",
            "r",
            "--repair-id",
            "leading-bom",
            "--gate",
            "prompt",
            "--mode",
            "default",
            "--attempt",
            "a",
            "--round",
            "1",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert!(parse_args(&required).is_ok());
        let mut duplicate = required;
        duplicate.extend(["--round".to_string(), "2".to_string()]);
        assert!(parse_args(&duplicate).is_err());
    }

    #[test]
    fn historical_planning_proof_allows_presentation_repair_and_memory_restamp() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".dev/plans")).unwrap();
        let source_path = root.join(".dev/plans/plan.md");
        let draft_path = root.join(".dev/plans/plan.en.md");
        let task_id = format!("T-{:02}", 10);
        let source_body = format!(
            "# Tasks\n- [ ] {task_id} task\n\nCurrent Task: {task_id}\n\n### Deviations\n\n| Step | Plan Said | Actually Did | Why |\n| --- | --- | --- | --- |\n"
        );
        let draft_body = "# Tasks\n- [ ] task\n";
        let localized = format!(
            "{source_body}<!-- gal:planning-authority\nsemantic-draft: .dev/plans/plan.en.md\nplanLanguage: zh-TW\ndraft-hash: {}\nrendered-source-hash: {}\nprompt-hash: none\nequivalence-verdict: pending\n-->\n",
            super::super::planning_authority::rendered_source_hash(draft_body),
            super::super::planning_authority::rendered_source_hash(&source_body),
        );
        std::fs::write(&source_path, &localized).unwrap();
        std::fs::write(&draft_path, draft_body).unwrap();
        let hooks = root.join("empty-hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "core.hooksPath", hooks.to_str().unwrap()])
                .unwrap()
                .status
                .success()
        );
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(
            root,
            &["add", "--", ".dev/plans/plan.md", ".dev/plans/plan.en.md"]
        )
        .unwrap()
        .status
        .success());
        let commit = git(root, &["commit", "-m", "baseline"]).unwrap();
        assert!(
            commit.status.success(),
            "{}",
            String::from_utf8_lossy(&commit.stderr)
        );

        let prior = recover_planning_record(
            root,
            ".dev/plans/plan.md",
            &super::super::planning_authority::rendered_source_hash(&localized),
        )
        .unwrap();
        let repaired = candidate(&RepairId::HeadingLevel, localized.as_bytes()).unwrap();
        assert_eq!(
            protected_projection(&RepairId::HeadingLevel, prior.as_bytes()).unwrap(),
            protected_projection(&RepairId::HeadingLevel, &repaired).unwrap()
        );
        let repaired = append_deviation_audit_row(&repaired, "heading-level").unwrap();
        let stamped =
            super::super::planning_authority::stamp_metadata_bytes(&repaired, draft_body).unwrap();
        super::super::planning_authority::verify_stamp_freshness(&stamped, None).unwrap();
        let metadata = super::super::planning_authority::parse_localized_meta(&stamped).unwrap();
        assert_eq!(metadata.equivalence_verdict, "pending");
        assert_eq!(metadata.prompt_hash, "none");
        assert_eq!(
            metadata.rendered_source_hash,
            super::super::planning_authority::rendered_source_hash(&stamped)
        );
        let late_audit = append_deviation_audit_row(stamped.as_bytes(), "heading-level").unwrap();
        assert!(
            super::super::planning_authority::verify_stamp_freshness(&late_audit, None).is_err()
        );
    }

    #[test]
    fn missing_historical_planning_proof_is_rejected() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(recover_planning_record(root, "absent.md", "missing").is_err());
    }

    #[test]
    fn equivalent_stamp_recovers_prompt_without_deleted_draft() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".dev/plans")).unwrap();
        let hooks = root.join("empty-hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let task_id = format!("T-{:02}", 20);
        let source_body = format!(
            "# Tasks\n- [ ] {task_id} task\n\nCurrent Task: {task_id}\n\n### Deviations\n\n| Step | Plan Said | Actually Did | Why |\n| --- | --- | --- | --- |\n"
        );
        let prompt = "# Prompt\nAccepted prompt\n";
        let source = format!(
            "{source_body}<!-- gal:planning-authority\nsemantic-draft: .dev/plans/plan.en.md\nplanLanguage: zh-TW\ndraft-hash: preserved-draft-hash\nrendered-source-hash: {}\nprompt-hash: {}\nequivalence-verdict: EQUIVALENT\n-->\n",
            super::super::planning_authority::rendered_source_hash(&source_body),
            super::super::planning_authority::rendered_source_hash(prompt),
        );
        std::fs::write(root.join(".dev/plans/plan.md"), &source).unwrap();
        std::fs::write(root.join(".dev/plans/plan.prompt.md"), prompt).unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "core.hooksPath", hooks.to_str().unwrap()])
                .unwrap()
                .status
                .success()
        );
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(
            root,
            &[
                "add",
                "--",
                ".dev/plans/plan.md",
                ".dev/plans/plan.prompt.md"
            ]
        )
        .unwrap()
        .status
        .success());
        let commit = git(root, &["commit", "-m", "baseline"]).unwrap();
        assert!(
            commit.status.success(),
            "{}",
            String::from_utf8_lossy(&commit.stderr)
        );

        let prior_prompt = recover_planning_record(
            root,
            ".dev/plans/plan.prompt.md",
            &super::super::planning_authority::rendered_source_hash(prompt),
        )
        .unwrap();
        let repaired = candidate(&RepairId::HeadingLevel, source.as_bytes()).unwrap();
        let repaired = append_deviation_audit_row(&repaired, "heading-level").unwrap();
        let stamped =
            super::super::planning_authority::refresh_equivalence_bytes(&repaired, &prior_prompt)
                .unwrap();
        super::super::planning_authority::verify_stamp_freshness(&stamped, Some(prompt)).unwrap();
        let metadata = super::super::planning_authority::parse_localized_meta(&stamped).unwrap();
        assert_eq!(metadata.draft_hash, "preserved-draft-hash");
        assert_eq!(metadata.equivalence_verdict, "EQUIVALENT");
        assert_eq!(
            metadata.rendered_source_hash,
            super::super::planning_authority::rendered_source_hash(&stamped)
        );
        assert_eq!(
            metadata.prompt_hash,
            super::super::planning_authority::rendered_source_hash(prompt)
        );
        assert!(!root.join(".dev/plans/plan.en.md").exists());
    }

    #[test]
    fn failed_publication_restores_only_its_bytes_and_index_entry() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let target = root.join("doc.md");
        let original = b"original\n";
        let candidate = b"repaired\n";
        std::fs::write(&target, original).unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["add", "--", "doc.md"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["commit", "-m", "baseline"])
            .unwrap()
            .status
            .success());
        let error = publish_and_commit_with(
            root,
            &target,
            "doc.md",
            original,
            candidate,
            "prompt",
            &[],
            || Err("injected failure".into()),
        )
        .unwrap_err();
        assert!(error.contains("original bytes and index restored"));
        assert_eq!(std::fs::read(&target).unwrap(), original);
        let status = git(root, &["status", "--porcelain=v1"]).unwrap();
        assert!(status.stdout.is_empty());
    }

    #[test]
    fn bracket_target_stages_literally_and_preserves_unrelated_staged_entry() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let relative = "[ab].prompt.md";
        let target = root.join(relative);
        let original = b"original\n";
        let candidate = b"repaired\n";
        std::fs::write(&target, original).unwrap();
        std::fs::write(root.join("a.prompt.md"), b"a baseline\n").unwrap();
        std::fs::write(root.join("b.prompt.md"), b"b baseline\n").unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["add", "-A"]).unwrap().status.success());
        assert!(git(root, &["commit", "-m", "baseline"])
            .unwrap()
            .status
            .success());

        let error = publish_and_commit_with(
            root,
            &target,
            relative,
            original,
            candidate,
            "prompt",
            &[],
            || {
                std::fs::write(root.join("a.prompt.md"), b"staged unrelated\n")
                    .map_err(|e| e.to_string())?;
                let staged = git(root, &["add", "--", "a.prompt.md"])?;
                if !staged.status.success() {
                    return Err("could not stage unrelated probe path".into());
                }
                std::fs::write(root.join("b.prompt.md"), b"concurrent edit\n")
                    .map_err(|e| e.to_string())?;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(
            error.contains("path set differs from declared repair paths"),
            "{error}"
        );
        assert_eq!(std::fs::read(&target).unwrap(), original);
        assert_eq!(
            std::fs::read(root.join("a.prompt.md")).unwrap(),
            b"staged unrelated\n"
        );
        assert_eq!(
            std::fs::read(root.join("b.prompt.md")).unwrap(),
            b"concurrent edit\n"
        );
        assert!(ensure_exact_paths(root, &["a.prompt.md"], "remaining staged").is_ok());
        let committed = git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "-z",
                "HEAD",
            ],
        )
        .unwrap();
        assert!(nul_paths(&committed.stdout).unwrap().is_empty());
        let target_staged = git(root, &["diff", "--cached", "--quiet", "--", relative]).unwrap();
        assert!(target_staged.status.success());
    }

    #[test]
    fn branch_race_refuses_and_preserves_unrelated_commit_and_clean_index() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let target = root.join("doc.md");
        let unrelated = root.join("unrelated.md");
        let original = b"original\n";
        let candidate = b"repaired\n";
        std::fs::write(&target, original).unwrap();
        std::fs::write(&unrelated, b"baseline unrelated\n").unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["add", "-A"]).unwrap().status.success());
        assert!(git(root, &["commit", "-m", "baseline"])
            .unwrap()
            .status
            .success());
        let previous_head = String::from_utf8(git(root, &["rev-parse", "HEAD"]).unwrap().stdout)
            .unwrap()
            .trim()
            .to_string();

        let error = publish_and_commit_with(
            root,
            &target,
            "doc.md",
            original,
            candidate,
            "prompt",
            &[],
            || {
                std::fs::write(&unrelated, b"unrelated commit bytes\n")
                    .map_err(|error| error.to_string())?;
                let target_blob = String::from_utf8(
                    git(root, &["rev-parse", &format!("{previous_head}:doc.md")])?.stdout,
                )
                .map_err(|error| error.to_string())?
                .trim()
                .to_string();
                let unrelated_blob = String::from_utf8(
                    git(root, &["hash-object", "-w", "unrelated.md"])?.stdout,
                )
                .map_err(|error| error.to_string())?
                .trim()
                .to_string();
                let tree_input = format!(
                    "100644 blob {target_blob}\tdoc.md\n100644 blob {unrelated_blob}\tunrelated.md\n"
                );
                let mut tree_command = Command::new("git")
                    .current_dir(root)
                    .arg("mktree")
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|error| error.to_string())?;
                use std::io::Write;
                let mut tree_input_pipe = tree_command
                    .stdin
                    .take()
                    .ok_or("could not open tree input")?;
                tree_input_pipe
                    .write_all(tree_input.as_bytes())
                    .map_err(|error| error.to_string())?;
                drop(tree_input_pipe);
                let tree_output = tree_command
                    .wait_with_output()
                    .map_err(|error| error.to_string())?;
                if !tree_output.status.success() {
                    return Err(format!(
                        "could not create unrelated race tree: {}",
                        String::from_utf8_lossy(&tree_output.stderr).trim()
                    ));
                }
                let tree = String::from_utf8_lossy(&tree_output.stdout)
                    .trim()
                    .to_string();
                let commit = git(
                    root,
                    &[
                        "commit-tree",
                        &tree,
                        "-p",
                        &previous_head,
                        "-m",
                        "unrelated concurrent commit",
                    ],
                )?;
                if !commit.status.success() {
                    return Err("could not create unrelated race commit".into());
                }
                let concurrent_head = String::from_utf8_lossy(&commit.stdout).trim().to_string();
                let moved = git(
                    root,
                    &["update-ref", "HEAD", &concurrent_head, &previous_head],
                )?;
                if !moved.status.success() {
                    return Err("could not advance branch in race probe".into());
                }
                let refreshed = git(root, &["read-tree", &concurrent_head])?;
                if !refreshed.status.success() {
                    return Err("could not refresh index for unrelated commit".into());
                }
                let staged_repair = git(root, &["add", "--", "doc.md"])?;
                if !staged_repair.status.success() {
                    return Err("could not retain only the repair index entry".into());
                }
                Ok(())
            },
        )
        .unwrap_err();

        let observed_head = String::from_utf8(git(root, &["rev-parse", "HEAD"]).unwrap().stdout)
            .unwrap()
            .trim()
            .to_string();
        assert_ne!(observed_head, previous_head);
        assert!(error.contains("repair branch update refused"), "{error}");
        assert!(error.contains(&observed_head), "{error}");
        let unrelated_paths = git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "-z",
                &previous_head,
                &observed_head,
            ],
        )
        .unwrap();
        assert_eq!(
            nul_paths(&unrelated_paths.stdout).unwrap(),
            ["unrelated.md"]
        );
        assert_eq!(std::fs::read(&target).unwrap(), original);
        assert_eq!(
            std::fs::read(&unrelated).unwrap(),
            b"unrelated commit bytes\n"
        );
        assert!(git(root, &["diff", "--cached", "--quiet"])
            .unwrap()
            .status
            .success());
    }

    #[test]
    fn dirty_target_and_staged_index_refuse_publication() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let target = root.join("doc.md");
        std::fs::write(&target, b"clean\n").unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["add", "--", "doc.md"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["commit", "-m", "baseline"])
            .unwrap()
            .status
            .success());
        std::fs::write(&target, b"dirty\n").unwrap();
        assert!(ensure_clean_target_and_index(root, "doc.md")
            .unwrap_err()
            .contains("dirty repair target"));
        assert_eq!(std::fs::read(&target).unwrap(), b"dirty\n");
        std::fs::write(&target, b"staged\n").unwrap();
        assert!(git(root, &["add", "--", "doc.md"])
            .unwrap()
            .status
            .success());
        std::fs::write(&target, b"clean\n").unwrap();
        assert!(ensure_clean_target_and_index(root, "doc.md")
            .unwrap_err()
            .contains("staged entries"));
    }

    #[test]
    fn stale_receipt_target_refuses_without_writing() {
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join("doc.md");
        let original = b"current bytes\n";
        std::fs::write(&target, original).unwrap();
        let receipt = format!("repair_target: doc.md sha256:{}\n", "0".repeat(64));
        let root = temp.path().canonicalize().unwrap();
        assert!(parse_receipt_targets(&receipt, &root)
            .unwrap_err()
            .contains("stale"));
        assert_eq!(std::fs::read(&target).unwrap(), original);
    }

    #[test]
    fn lease_conflict_releases_previously_acquired_targets() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let first = root.join("a.md");
        let second = root.join("b.md");
        std::fs::write(&first, b"a").unwrap();
        std::fs::write(&second, b"b").unwrap();
        let mut occupied =
            dispatch::dispatch::acquire_prompt_writer_lease(root, &second, None, "held").unwrap();
        let targets = vec![
            ("a.md".into(), String::new(), vec![b'a']),
            ("b.md".into(), String::new(), vec![b'b']),
        ];
        assert!(acquire_leases(root, &targets, "attempt")
            .err()
            .unwrap()
            .contains("lease conflict"));
        let first_lease =
            dispatch::dispatch::acquire_prompt_writer_lease(root, &first, None, "retry");
        assert!(first_lease.is_ok());
        let mut first_lease = first_lease.unwrap();
        first_lease.release_confirmed().unwrap();
        occupied.release_confirmed().unwrap();
        let mut second_lease =
            dispatch::dispatch::acquire_prompt_writer_lease(root, &second, None, "retry").unwrap();
        second_lease.release_confirmed().unwrap();
    }

    #[test]
    fn mapped_bom_repair_commits_once_while_all_target_leases_are_held() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let target = root.join("doc.md");
        let sibling = root.join("plan.md");
        let original = b"\xef\xbb\xbf## Tasks\n- [ ] example task\n";
        std::fs::write(&target, original).unwrap();
        std::fs::write(&sibling, b"sibling\n").unwrap();
        assert!(git(root, &["init"]).unwrap().status.success());
        assert!(
            git(root, &["config", "user.email", "repair@example.invalid"])
                .unwrap()
                .status
                .success()
        );
        assert!(git(root, &["config", "user.name", "Repair Probe"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["add", "--", "doc.md", "plan.md"])
            .unwrap()
            .status
            .success());
        assert!(git(root, &["commit", "-m", "baseline"])
            .unwrap()
            .status
            .success());

        let targets = vec![
            (
                "doc.md".to_string(),
                format!("{:x}", sha2::Sha256::digest(original)),
                original.to_vec(),
            ),
            (
                "plan.md".to_string(),
                format!("{:x}", sha2::Sha256::digest(b"sibling\n")),
                b"sibling\n".to_vec(),
            ),
        ];
        let commit_count_before = git(root, &["rev-list", "--count", "HEAD"]).unwrap();
        let commit_count_before = String::from_utf8(commit_count_before.stdout)
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap();
        let leases = acquire_leases(root, &targets, "repair-attempt").unwrap();
        let repaired = candidate(&RepairId::LeadingBom, original).unwrap();
        publish_and_commit_with(
            root,
            &target,
            "doc.md",
            original,
            &repaired,
            "prompt",
            &leases,
            || {
                for relative in ["doc.md", "plan.md"] {
                    assert!(dispatch::dispatch::acquire_prompt_writer_lease(
                        root,
                        &root.join(relative),
                        None,
                        "competing-writer",
                    )
                    .is_err());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), repaired);
        let changed = git(
            root,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
        )
        .unwrap();
        assert_eq!(String::from_utf8(changed.stdout).unwrap().trim(), "doc.md");
        let commit_count_after = git(root, &["rev-list", "--count", "HEAD"]).unwrap();
        assert_eq!(
            String::from_utf8(commit_count_after.stdout)
                .unwrap()
                .trim()
                .parse::<usize>()
                .unwrap(),
            commit_count_before + 1
        );
        let mut leases = leases;
        release_leases(&mut leases).unwrap();
        for (relative, _, _) in &targets {
            let mut lease = dispatch::dispatch::acquire_prompt_writer_lease(
                root,
                &root.join(relative),
                None,
                "after-release",
            )
            .unwrap();
            lease.release_confirmed().unwrap();
        }
        assert!(candidate(&RepairId::LeadingBom, &repaired).is_err());
    }

    #[test]
    fn protected_record_changes_are_rejected_and_deviation_rows_stay_identical() {
        let original = b"## Requirements\n- [ ] requirement-alpha\n\n## Approval\nAPPROVE\n\n## Review Results\nVerdict: CLEAR\n\n## Tasks\n- [ ] task-alpha\n- [ ] task-beta\n\n## Test Plan\n| test-alpha | command: cargo test | crates/a.rs |\n\n## Deviations\n| deviation-alpha | existing row |\n\nUnknown prose.\n";
        let baseline = protected_projection(&RepairId::HeadingLevel, original).unwrap();
        let deviation_rows = b"| deviation-alpha | existing row |";
        assert!(baseline
            .windows(deviation_rows.len())
            .any(|window| window == deviation_rows));
        let changes: [&[u8]; 8] = [
            b"requirement-alpha",
            b"APPROVE",
            b"Verdict: CLEAR",
            b"- [ ] task-alpha\n- [ ] task-beta",
            b"test-alpha | command: cargo test",
            b"crates/a.rs",
            b"Unknown prose.",
            b"command: cargo test",
        ];
        for marker in changes {
            let mut candidate = original.to_vec();
            let position = candidate
                .windows(marker.len())
                .position(|window| window == marker)
                .unwrap();
            if marker.starts_with(b"- [ ] task-alpha") {
                let replacement = b"- [ ] task-beta\n- [ ] task-alpha";
                candidate.splice(
                    position..position + marker.len(),
                    replacement.iter().copied(),
                );
            } else if marker == b"command: cargo test" {
                candidate.splice(
                    position..position + marker.len(),
                    b"command:cargo test".iter().copied(),
                );
            } else {
                candidate[position..position + marker.len()].fill(b'X');
            }
            assert!(
                ensure_protected_records_unchanged(&RepairId::HeadingLevel, original, &candidate)
                    .is_err(),
                "protected change was accepted: {}",
                String::from_utf8_lossy(marker)
            );
            assert!(candidate
                .windows(deviation_rows.len())
                .any(|window| window == deviation_rows));
        }
    }

    // --- catalog nomination probes ---

    const PROBE_ATTEMPT: &str = "probe-attempt";
    const ALL_REPAIRS: [&str; 5] = [
        "leading-bom",
        "heading-level",
        "bullet-shape",
        "table-shape",
        "retry-field-shape",
    ];
    const PROMPT_PATH: &str = ".dev/plans/probe.prompt.md";
    const SOURCE_PATH: &str = ".dev/plans/probe.md";

    fn failed(check: &str) -> super::super::finalize_check::CheckOutcome {
        super::super::finalize_check::CheckOutcome {
            name: check.into(),
            command: None,
            state: super::super::finalize_check::CheckState::Fail,
            summary: "failed".into(),
        }
    }

    fn probe_context(gate: &str, mode: &str, path: &str, bytes: &[u8]) -> RepairContext {
        RepairContext {
            gate: gate.into(),
            mode: mode.into(),
            attempt_id: PROBE_ATTEMPT.into(),
            round: 0,
            active_run_id: None,
            targets: vec![RepairTarget {
                path: path.into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                raw_bytes: bytes.to_vec(),
            }],
            factual_inputs: Vec::new(),
        }
    }

    fn probe_receipt(gate: &str, mode: &str, line: &str) -> String {
        format!(
            "repair_context: bound\ngate: {gate}\nmode: {mode}\ngate_attempt_id: {PROBE_ATTEMPT}\nrepair_round: 0\n{line}\n"
        )
    }

    /// Nominate through the shared receipt classifier, render its declaration
    /// into a receipt, and run the helper's own acceptance on the same bytes.
    fn nominate_and_accept(
        gate: &str,
        mode: &str,
        check: &str,
        path: &str,
        bytes: &[u8],
    ) -> (&'static str, Vec<u8>) {
        let root = tempfile::tempdir().unwrap();
        if check == "finalize-review-shape" {
            let evidence_path = root.path().join(PROMPT_PATH);
            std::fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
            std::fs::write(evidence_path, "fixture evidence\n").unwrap();
        }
        assert!(
            !check_passes(gate, check, bytes, root.path()),
            "fixture must fail {check} before repair"
        );
        let context = probe_context(gate, mode, path, bytes);
        let (repair, target) = super::super::finalize_check::classify_static_repair(
            &context,
            &failed(check),
            root.path(),
        )
        .expect("an eligible target must be nominated");
        assert_eq!(target, path);
        let receipt = probe_receipt(gate, mode, &format_disposition_line(check, repair, target));
        let candidate = helper_accepts(
            &receipt,
            repair,
            gate,
            mode,
            PROBE_ATTEMPT,
            target,
            bytes,
            root.path(),
        )
        .expect("the helper must accept the receipt's nomination");
        assert!(check_passes(gate, check, &candidate, root.path()));
        (repair, candidate)
    }

    /// No repair is nominated, and the helper refuses even a forged
    /// declaration of every catalogued repair for this target.
    fn stays_owner(gate: &str, mode: &str, check: &str, path: &str, bytes: &[u8]) {
        let root = tempfile::tempdir().unwrap();
        let context = probe_context(gate, mode, path, bytes);
        assert_eq!(
            super::super::finalize_check::classify_static_repair(
                &context,
                &failed(check),
                root.path()
            ),
            None
        );
        for repair in ALL_REPAIRS {
            let receipt = probe_receipt(gate, mode, &format_disposition_line(check, repair, path));
            assert!(
                helper_accepts(
                    &receipt,
                    repair,
                    gate,
                    mode,
                    PROBE_ATTEMPT,
                    path,
                    bytes,
                    root.path()
                )
                .is_err(),
                "forged {repair} declaration was accepted"
            );
        }
    }

    #[test]
    fn leading_bom_is_nominated_for_prompt_tasks_heading_and_accepted() {
        let task = format!("T-{:02}", 1);
        let bytes = format!("\u{feff}## Tasks\n- [ ] {task} probe\n").into_bytes();
        let (repair, candidate) =
            nominate_and_accept("prompt", "default", "tasks-heading", PROMPT_PATH, &bytes);
        assert_eq!(repair, "leading-bom");
        assert_eq!(candidate, &bytes[3..]);
    }

    #[test]
    fn leading_bom_without_a_bom_or_heading_stays_owner() {
        stays_owner(
            "prompt",
            "default",
            "tasks-heading",
            PROMPT_PATH,
            b"## Status\n\nno tasks heading\n",
        );
    }

    #[test]
    fn heading_level_is_nominated_for_prompt_tasks_heading_and_accepted() {
        let task = format!("T-{:02}", 1);
        let bytes = format!("# Tasks\n- [ ] {task} probe\n").into_bytes();
        let (repair, candidate) =
            nominate_and_accept("prompt", "default", "tasks-heading", PROMPT_PATH, &bytes);
        assert_eq!(repair, "heading-level");
        assert_eq!(
            candidate,
            format!("## Tasks\n- [ ] {task} probe\n").as_bytes()
        );
    }

    #[test]
    fn heading_level_with_duplicate_headings_stays_owner() {
        let task = format!("T-{:02}", 1);
        let bytes = format!("# Tasks\n# Tasks\n- [ ] {task} probe\n").into_bytes();
        stays_owner("prompt", "default", "tasks-heading", PROMPT_PATH, &bytes);
    }

    #[test]
    fn bullet_shape_is_nominated_for_preflight_tasks_well_formed_and_accepted() {
        let short = format!("T-{}", 1);
        let canonical = format!("T-{:02}", 1);
        let test_id = format!("TP-{:02}", 1);
        let bytes = format!(
            "## Status\n\nCurrent Task: {canonical}\n\n## Tasks\n\n-  [ ]  {short}: colon task stays exact\n\n## Test Plan\n\n| ID | Description | Covers |\n| --- | --- | --- |\n| {test_id} | probe | {short} |\n"
        )
        .into_bytes();
        let (repair, candidate) = nominate_and_accept(
            PREFLIGHT_GATE,
            PREFLIGHT_ORDINARY_MODE,
            "tasks-well-formed",
            PROMPT_PATH,
            &bytes,
        );
        assert_eq!(repair, "bullet-shape");
        let text = String::from_utf8(candidate).unwrap();
        assert!(text.contains(&format!("- [ ] {canonical}: colon task stays exact\n")));
        assert!(text.contains(&format!("| {test_id} | probe | {canonical} |\n")));
    }

    #[test]
    fn bullet_shape_with_unrecognized_or_colliding_references_stays_owner() {
        let short = format!("T-{}", 1);
        let canonical = format!("T-{:02}", 1);
        let prose_reference =
            format!("## Status\n\nCurrent Task: {short}\n\n## Tasks\n\n- [ ] {short} probe\n");
        stays_owner(
            PREFLIGHT_GATE,
            PREFLIGHT_ORDINARY_MODE,
            "tasks-well-formed",
            PROMPT_PATH,
            prose_reference.as_bytes(),
        );
        let collision = format!("## Tasks\n\n- [ ] {short} one\n- [ ] {canonical} two\n");
        stays_owner(
            "refining",
            "default",
            "id-well-formed",
            SOURCE_PATH,
            collision.as_bytes(),
        );
    }

    #[test]
    fn table_shape_is_nominated_for_refining_coverage_and_accepted() {
        let task = format!("T-{:02}", 1);
        let test_id = format!("TP-{:02}", 1);
        let bytes = format!(
            "## Tasks\n\n- [ ] {task} probe\n\n## Test Plan\n\n| ID | Description | Covers |\n| - - | :-- | --- |\n| {test_id} | probe | {task} |\n"
        )
        .into_bytes();
        let (repair, candidate) = nominate_and_accept(
            "refining",
            "default",
            "test-plan-coverage",
            SOURCE_PATH,
            &bytes,
        );
        assert_eq!(repair, "table-shape");
        assert!(String::from_utf8(candidate)
            .unwrap()
            .contains("| --- | :--- | --- |\n"));
    }

    #[test]
    fn table_shape_is_nominated_for_finalize_review_shape_on_the_prompt() {
        let requirement = format!("R{}", 1);
        let evidence = format!("{PROMPT_PATH}:1");
        let bytes = format!(
            "## Requirements\n\n- [ ] {requirement} — probe requirement\n\n## Review Results\n\n### Finalize Review 2026-10-27\n\n| Requirement | L1 | L2 | L3 | L4 |\n| - - | --- | --- | --- | --- |\n| {requirement} | PASS — hunk and probe | PASS — file | PASS — wired | PASS — {{\"new-trust-boundaries\":{{\"result\":\"pass\",\"reason\":\"reviewed\",\"evidence\":[\"{evidence}\"]}},\"guard-preservation\":{{\"result\":\"pass\",\"reason\":\"preserved\",\"evidence\":[\"{evidence}\"]}}}} |\n\nReview Independence: full\n"
        )
        .into_bytes();
        let (repair, _) = nominate_and_accept(
            "finalize",
            "hygiene-only",
            "finalize-review-shape",
            PROMPT_PATH,
            &bytes,
        );
        assert_eq!(repair, "table-shape");
        stays_owner(
            "finalize",
            "full",
            "finalize-review-shape",
            PROMPT_PATH,
            &bytes,
        );
        stays_owner(
            "finalize",
            "hygiene-only",
            "finalize-review-shape",
            SOURCE_PATH,
            &bytes,
        );
    }

    #[test]
    fn table_shape_with_an_inferred_delimiter_cell_stays_owner() {
        let task = format!("T-{:02}", 1);
        let test_id = format!("TP-{:02}", 1);
        let bytes = format!(
            "## Tasks\n\n- [ ] {task} probe\n\n## Test Plan\n\n| ID | Description | Covers |\n| -- | --- | |\n| {test_id} | probe | {task} |\n"
        );
        stays_owner(
            "refining",
            "default",
            "test-plan-coverage",
            SOURCE_PATH,
            bytes.as_bytes(),
        );
    }

    fn retry_fixture(status_line: &str) -> String {
        let task = format!("T-{:02}", 6);
        format!(
            "## Status\n\nCurrent Task: {task}\n\n### Handoff Notes\n\n#### Retry Handoff — {task} / IMPLEMENT\n\n{status_line}\n- Problem: issue\n- Next human step: action\n\n### Deviations\n\n| Step | Plan Said | Actually Did | Why |\n| --- | --- | --- | --- |\n\n## Tasks\n\n- [ ] {task} probe\n"
        )
    }

    #[test]
    fn retry_field_shape_is_nominated_for_preflight_retry_handoff_and_accepted() {
        let bytes = retry_fixture("- Status: OPEN — waiting on the owner").into_bytes();
        let (repair, candidate) = nominate_and_accept(
            PREFLIGHT_GATE,
            PREFLIGHT_ORDINARY_MODE,
            "retry-handoff-shape",
            PROMPT_PATH,
            &bytes,
        );
        assert_eq!(repair, "retry-field-shape");
        assert!(String::from_utf8(candidate)
            .unwrap()
            .contains("- Status: OPEN\n- Status note: — waiting on the owner\n- Problem: issue\n"));
    }

    #[test]
    fn retry_field_shape_with_an_unknown_status_stays_owner() {
        let bytes = retry_fixture("- Status: PENDING — waiting on the owner").into_bytes();
        stays_owner(
            PREFLIGHT_GATE,
            PREFLIGHT_ORDINARY_MODE,
            "retry-handoff-shape",
            PROMPT_PATH,
            &bytes,
        );
    }

    #[test]
    fn terminal_reverify_preflight_never_nominates_a_repair() {
        let bytes = retry_fixture("- Status: OPEN — waiting on the owner").into_bytes();
        stays_owner(
            PREFLIGHT_GATE,
            "terminal-reverify",
            "retry-handoff-shape",
            PROMPT_PATH,
            &bytes,
        );
    }

    #[test]
    fn non_metadata_repair_appends_one_deviation_row_after_the_equality_proof() {
        let bytes = retry_fixture("- Status: OPEN — waiting on the owner");
        let task = format!("T-{:02}", 6);
        let repaired = retry_field_candidate(bytes.as_bytes()).unwrap();
        let with_row =
            append_repair_deviation_row(&repaired, "retry-field-shape", "retry-handoff-shape")
                .unwrap();
        let text = String::from_utf8(with_row).unwrap();
        assert_eq!(text.matches("| Formatting-only repair |").count(), 1);
        assert!(text.contains(&format!(
            "| {task} | Formatting-only repair | Applied retry-field-shape for retry-handoff-shape |"
        )));
        let without_table = format!("## Tasks\n\n- [ ] {task} probe\n");
        assert_eq!(
            append_repair_deviation_row(without_table.as_bytes(), "bullet-shape", "x").unwrap(),
            without_table.as_bytes()
        );
    }

    #[test]
    fn shape_projections_reject_protected_value_changes() {
        let short = format!("T-{}", 1);
        let canonical = format!("T-{:02}", 1);
        let other = format!("T-{:02}", 2);
        let bullet = format!("## Tasks\n- [ ] {short} keep\n");
        for changed in [
            format!("## Tasks\n- [x] {canonical} keep\n"),
            format!("## Tasks\n- [ ] {other} keep\n"),
            format!("## Tasks\n- [ ] {canonical}  keep\n"),
        ] {
            assert!(ensure_protected_records_unchanged(
                &RepairId::BulletShape,
                bullet.as_bytes(),
                changed.as_bytes()
            )
            .is_err());
        }
        let table = "| ID | Covers |\n| - - | :-- |\n| a | b |\n";
        for changed in [
            "| ID | Covers |\n| --- | --- |\n| a | b |\n",
            "| ID | Covers |\n| --- | :--- |\n| a | c |\n",
        ] {
            assert!(ensure_protected_records_unchanged(
                &RepairId::TableShape,
                table.as_bytes(),
                changed.as_bytes()
            )
            .is_err());
        }
        let retry = retry_fixture("- Status: OPEN — note");
        let changed = retry.replace(
            "- Status: OPEN — note",
            "- Status: RESOLVED\n- Status note: — note",
        );
        assert!(ensure_protected_records_unchanged(
            &RepairId::RetryFieldShape,
            retry.as_bytes(),
            changed.as_bytes()
        )
        .is_err());
    }

    #[test]
    fn helper_refuses_a_format_row_when_the_same_receipt_has_an_owner_row() {
        let short = format!("T-{}", 1);
        let bytes = format!("## Tasks\n\n-  [ ]  {short} probe\n").into_bytes();
        let root = tempfile::tempdir().unwrap();
        let declaration = format_disposition_line("tasks-well-formed", "bullet-shape", PROMPT_PATH);
        let clean = probe_receipt(PREFLIGHT_GATE, PREFLIGHT_ORDINARY_MODE, &declaration);
        let accept = |receipt: &str| {
            helper_accepts(
                receipt,
                "bullet-shape",
                PREFLIGHT_GATE,
                PREFLIGHT_ORDINARY_MODE,
                PROBE_ATTEMPT,
                PROMPT_PATH,
                &bytes,
                root.path(),
            )
        };
        assert!(accept(&clean).is_ok());
        let mixed = format!("{clean}- retry-handoff-shape: owner (repair_id: none)\n");
        let error = accept(&mixed).unwrap_err();
        assert!(error.contains("owner row"), "{error}");
    }

    #[test]
    fn bullet_projection_protects_test_plan_cells_outside_covers() {
        let short = format!("T-{}", 1);
        let canonical = format!("T-{:02}", 1);
        let test_id = format!("TP-{:02}", 1);
        let header = "## Test Plan\n\n| ID | Description | Covers |\n| --- | --- | --- |\n";
        let original = format!(
            "## Tasks\n\n- [ ] {short} probe\n\n{header}| {test_id} | see {short} | {short} |\n"
        );
        let covers_only = format!(
            "## Tasks\n\n- [ ] {canonical} probe\n\n{header}| {test_id} | see {short} | {canonical} |\n"
        );
        ensure_protected_records_unchanged(
            &RepairId::BulletShape,
            original.as_bytes(),
            covers_only.as_bytes(),
        )
        .unwrap();
        let description_changed = format!(
            "## Tasks\n\n- [ ] {canonical} probe\n\n{header}| {test_id} | see {canonical} | {canonical} |\n"
        );
        assert!(ensure_protected_records_unchanged(
            &RepairId::BulletShape,
            original.as_bytes(),
            description_changed.as_bytes()
        )
        .is_err());
    }
}
