use std::fs;
use std::path::{Path, PathBuf};

pub fn gal_invocations(content: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut in_fence = false;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            // Inside a fence, match `gal ` only where the preceding character is absent
            // or is none of '/', '$', '-', or a word character.
            let pattern = "gal ";
            let mut start_idx = 0;
            while let Some(rel_pos) = line[start_idx..].find("gal ") {
                let pos = start_idx + rel_pos;
                let preceded_excluded = if pos == 0 {
                    false
                } else {
                    let prev_char = line[..pos].chars().next_back().unwrap();
                    prev_char == '/'
                        || prev_char == '$'
                        || prev_char == '-'
                        || prev_char.is_alphanumeric()
                        || prev_char == '_'
                };

                if !preceded_excluded {
                    let remainder = &line[pos + pattern.len()..];
                    if let Some(raw_token) = remainder.split_whitespace().next() {
                        let token = raw_token.trim_end_matches('`');
                        if !token.starts_with('-') && !token.is_empty() {
                            results.push((line_num, token.to_string()));
                        }
                    }
                }
                start_idx = pos + pattern.len();
            }
        } else {
            // Outside a fence, match every inline-code span and keep those whose content begins `gal `.
            // An inline code span is between odd indices: 0 ` 1 ` 2 ` 3 ...
            // where 1, 3, 5 are inside backticks.
            for (idx, part) in line.split('`').enumerate() {
                if idx % 2 == 1 {
                    // Inside inline-code span
                    if let Some(remainder) = part.strip_prefix("gal ") {
                        if let Some(raw_token) = remainder.split_whitespace().next() {
                            let token = raw_token.trim_end_matches('`');
                            if !token.starts_with('-') && !token.is_empty() {
                                results.push((line_num, token.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

#[test]
fn test_tp01_gal_invocations_inline_and_fenced() {
    let fixture = "\
Here is an inline span `gal init` to test.
And here is regular prose run gal status which should not match.
```sh
gal planning-check x.md
```
";
    let invocations = gal_invocations(fixture);
    assert_eq!(
        invocations,
        vec![(1, "init".to_string()), (4, "planning-check".to_string()),],
        "gal_invocations extracts inline spans, fenced lines, and skips flags"
    );
}

#[test]
fn test_tp02_gal_invocations_exclusions() {
    let fixture = "\
Inline non-invocations: `$gal status`, `/gal-status`.
```sh
/gal finalize
```
";
    let invocations = gal_invocations(fixture);
    assert_eq!(invocations, Vec::<(usize, String)>::new());
}

#[test]
fn test_tp03_gal_invocations_flags_and_backticks() {
    let fixture_flag = "\
Run `gal --version` for info.
";
    assert_eq!(gal_invocations(fixture_flag), Vec::<(usize, String)>::new());

    let fixture_fabricated = "\
Line 1: `gal fake-subcommand-one`
Line 2: `gal fake-subcommand-two`
";
    assert_eq!(
        gal_invocations(fixture_fabricated),
        vec![
            (1, "fake-subcommand-one".to_string()),
            (2, "fake-subcommand-two".to_string()),
        ]
    );

    let fixture_fenced_inline = "\
```markdown
`gal finalize-check`
```
";
    assert_eq!(
        gal_invocations(fixture_fenced_inline),
        vec![(2, "finalize-check".to_string())]
    );
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read directory {}: {e}", dir.display()))
        .map(|res| res.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_markdown_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

#[test]
fn test_real_tree_contract_command_tokens() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR must have at least two ancestors to reach repo root")
        .to_path_buf();
    let commands_dir = repo_root.join("plugins").join("gal-core").join("commands");

    assert!(
        commands_dir.exists(),
        "plugins/gal-core/commands must exist in the repo checkout at {}: this test inspects the real contract tree",
        commands_dir.display()
    );

    let mut md_files = Vec::new();
    collect_markdown_files(&commands_dir, &mut md_files);

    let mut bad_subcommands = Vec::new();
    for file in md_files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("failed to read file {}: {e}", file.display()));
        let rel_path = file
            .strip_prefix(&repo_root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        for (line, token) in gal_invocations(&content) {
            if gal_engine::CommandKind::parse(&token).is_none() {
                bad_subcommands.push((rel_path.clone(), line, token));
            }
        }
    }

    let formatted: Vec<String> = bad_subcommands
        .iter()
        .map(|(p, line, tok)| format!("{p}:{line}: {tok}"))
        .collect();

    assert!(
        bad_subcommands.is_empty(),
        "contract names a gal subcommand the binary does not accept:\n{}",
        formatted.join("\n")
    );
}
