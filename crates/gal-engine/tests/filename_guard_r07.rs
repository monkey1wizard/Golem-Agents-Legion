use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_SUBSTRINGS: &[&str] = &["install", "setup", "update", "patch"];

#[test]
fn gal_engine_test_filenames_avoid_uac_trigger_words_without_asinvoker_manifest() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if crate_has_asinvoker_manifest(&crate_root) {
        return;
    }

    let tests_dir = crate_root.join("tests");
    let mut offenders = Vec::new();

    for entry in fs::read_dir(&tests_dir).expect("tests directory should be readable") {
        let entry = entry.expect("test directory entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("test filename should be valid UTF-8")
            .to_ascii_lowercase();

        if FORBIDDEN_SUBSTRINGS.iter().any(|needle| stem.contains(needle)) {
            offenders.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "gal-engine test filenames must avoid {:?} unless an asInvoker manifest exists; offenders: {:?}",
        FORBIDDEN_SUBSTRINGS,
        offenders
    );
}

fn crate_has_asinvoker_manifest(root: &Path) -> bool {
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.to_ascii_lowercase().contains("manifest") {
                continue;
            }

            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            if contents.contains("asInvoker") {
                return true;
            }
        }
    }

    false
}