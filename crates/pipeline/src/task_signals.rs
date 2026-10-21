//! Extract model-policy signals from a task and the project index.

use std::collections::BTreeSet;

use dispatch::{model_tier::TaskSignals, stage::Phase};

use crate::{
    task_spec::{extract_declared_file_paths_strict, extract_task_goal},
    PipelineError,
};

/// Build the task signals consumed by the host-subagent model policy.
pub fn extract_task_signals(
    prompt: &str,
    task_id: &str,
    project_index: &str,
    phase: Phase,
) -> Result<TaskSignals, PipelineError> {
    let task = extract_task_goal(prompt, task_id)?;
    let files = extract_declared_file_paths_strict(prompt, task_id);
    let protected_paths = extract_protected_paths(project_index);
    let crate_roots = files
        .iter()
        .filter_map(|file| crate_root(file))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(TaskSignals {
        phase,
        has_protected_path: files.iter().any(|file| {
            protected_paths
                .iter()
                .any(|protected| path_matches(file, protected))
        }),
        dependency_count: extract_dependency_count(&task),
        files,
        crate_roots,
        prior_failed_attempts: 0,
        implement: None,
        test: None,
    })
}

fn extract_protected_paths(project_index: &str) -> Vec<String> {
    let mut in_section = false;
    let mut paths = Vec::new();
    for line in project_index.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if in_section {
                break;
            }
            in_section = heading.trim() == "Protected Paths";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((_, description)) = line.split_once('`') else {
            continue;
        };
        if let Some((path, _)) = description.split_once('`') {
            let path = normalize_path(path);
            if !path.is_empty() {
                paths.push(path);
            }
        }
    }
    paths
}

fn extract_dependency_count(task: &str) -> usize {
    task.lines()
        .find_map(|line| {
            let (label, value) = line.trim().split_once(':')?;
            label
                .trim()
                .eq_ignore_ascii_case("- Depends")
                .then_some(value.trim())
        })
        .map(|value| {
            let value = value
                .char_indices()
                .find(|(index, character)| {
                    *character == '.'
                        && value[index + 1..]
                            .chars()
                            .next()
                            .is_none_or(char::is_whitespace)
                })
                .map(|(index, _)| &value[..index])
                .unwrap_or(value)
                .trim();
            if value.eq_ignore_ascii_case("none") || value.is_empty() {
                0
            } else {
                value
                    .split(',')
                    .filter(|dependency| !dependency.trim().is_empty())
                    .count()
            }
        })
        .unwrap_or(0)
}

fn crate_root(path: &str) -> Option<String> {
    let normalized = normalize_path(path);
    let mut components = normalized.split('/').map(str::to_owned);
    while let Some(component) = components.next() {
        if component == "crates" {
            return components.next();
        }
    }
    None
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_matches('`')
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_owned()
}

fn path_matches(file: &str, protected: &str) -> bool {
    let file = normalize_path(file);
    let protected = normalize_path(protected);
    let protected = protected.trim_end_matches('/');
    !protected.is_empty()
        && (file == protected
            || file
                .strip_prefix(protected)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(task: &str, files: &[&str], project: &str) -> TaskSignals {
        let listed = files
            .iter()
            .map(|file| format!("- `{file}`"))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "## Tasks\n\n- [ ] task-a {task}\n  - Files: {listed}\n\n## Files to Create or Modify\n\n{listed}\n"
        );
        extract_task_signals(&prompt, "task-a", project, Phase::Implement).unwrap()
    }

    #[test]
    fn task_signals_extracts_protected_paths_at_boundaries() {
        let project = "## Protected Paths\n- `plugins/gal-core/commands/` — protected\n";
        assert!(signals("change", &["plugins/gal-core/commands/a.md"], project).has_protected_path);
        assert!(
            !signals("change", &["plugins/gal-core/commands-extra/a.md"], project)
                .has_protected_path
        );
    }

    #[test]
    fn task_signals_extracts_depends_none_and_dependency_lists() {
        assert_eq!(
            signals("\n  - Depends: None", &["a.rs"], "").dependency_count,
            0
        );
        assert_eq!(
            signals("\n  - Depends: task-a, task-b", &["a.rs"], "").dependency_count,
            2
        );
    }

    #[test]
    fn task_signals_parses_execution_prompt_task_block() {
        let prompt = "## Tasks\n\n- [ ] task-a Example\n  - Files: `crates/pipeline/src/lib.rs`, `crates/dispatch/src/lib.rs`, `crates/pipeline/src/coordinator.rs`.\n  - Depends: None. Blast radius: 3 files, 2 crates.\n  - Change: Update signals.\n";
        let result = extract_task_signals(prompt, "task-a", "", Phase::Implement).unwrap();
        assert_eq!(
            result.files,
            [
                "crates/pipeline/src/lib.rs",
                "crates/dispatch/src/lib.rs",
                "crates/pipeline/src/coordinator.rs"
            ]
        );
        assert_eq!(result.crate_roots, ["dispatch", "pipeline"]);
        assert_eq!(result.dependency_count, 0);

        let prompt = prompt.replace("Depends: None", "Depends: task-b, task-c");
        let result = extract_task_signals(&prompt, "task-a", "", Phase::Implement).unwrap();
        assert_eq!(result.dependency_count, 2);
    }

    #[test]
    fn task_signals_counts_distinct_crate_roots() {
        let result = signals(
            "work",
            &[
                "crates/pipeline/src/lib.rs",
                "crates/pipeline/src/x.rs",
                "crates/dispatch/src/lib.rs",
            ],
            "",
        );
        assert_eq!(result.crate_roots, ["dispatch", "pipeline"]);
    }

    #[test]
    fn task_signals_model_tier_classifies_documentation_lists() {
        use dispatch::model_tier::{compute_tier, ModelTier};

        for file in ["README.md", "docs/a.txt", "docs/a.rst", "docs/a.adoc"] {
            assert_eq!(
                compute_tier(&signals("docs", &[file], "")).unwrap().tier,
                ModelTier::Light,
                "{file}"
            );
        }
        for file in [
            "packaging/publish-public.sh",
            "tools/writing/check.mjs",
            "config.toml",
            "unknown",
            "code.rs",
        ] {
            assert!(
                compute_tier(&signals("other", &[file], "")).unwrap().tier != ModelTier::Light,
                "{file}"
            );
        }
        assert_ne!(
            compute_tier(&signals("empty", &[], "")).unwrap().tier,
            ModelTier::Light
        );
    }
}
