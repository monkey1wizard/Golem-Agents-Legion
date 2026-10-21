#[path = "../src/commands/pipeline_execution.rs"]
mod pipeline_execution;

use sha2::{Digest, Sha256};
use std::{fs, path::Path, thread, time::Instant};
use tempfile::TempDir;

fn write_project(root: &Path, source: &str) -> std::path::PathBuf {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"gal\"\npath = \"src/main.rs\"\n").unwrap();
    fs::write(root.join("src/main.rs"), source).unwrap();
    root.join("Cargo.toml")
}

fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

#[test]
fn private_builds_serialize_publish_immutable_hashes_and_keep_binding_on_failure() {
    let fixture = TempDir::new().unwrap();
    let root = fixture.path().join("source");
    fs::create_dir_all(&root).unwrap();
    let manifest = write_project(&root, "fn main() { println!(\"generation-one\"); }\n");

    let started = Instant::now();
    let root_a = root.clone();
    let manifest_a = manifest.clone();
    let first = thread::spawn(move || {
        pipeline_execution::build_private_generation(&root_a, &manifest_a).unwrap()
    });
    let root_b = root.clone();
    let manifest_b = manifest.clone();
    let second = thread::spawn(move || {
        pipeline_execution::build_private_generation(&root_b, &manifest_b).unwrap()
    });
    let generation_a = first.join().unwrap();
    let generation_b = second.join().unwrap();
    let elapsed = started.elapsed();

    assert_eq!(
        generation_a, generation_b,
        "serialized process results must reuse one immutable generation"
    );
    let digest = hash(&generation_a);
    assert_eq!(
        generation_a
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        digest
    );
    assert!(
        root.join("target/gal-pipeline/cargo-target/debug").is_dir(),
        "Cargo must use the worktree-private target directory"
    );
    let original_bytes = fs::read(&generation_a).unwrap();

    let binding = root.join("binding.json");
    fs::write(&binding, b"previous-binding\n").unwrap();
    let failed_manifest = write_project(&root, "fn main( { this is invalid rust\n");
    let failure =
        pipeline_execution::build_private_generation(&root, &failed_manifest).unwrap_err();
    assert!(
        failure.to_string().contains("cargo build failed"),
        "failed Cargo process result: {failure}"
    );
    assert_eq!(
        fs::read(&binding).unwrap(),
        b"previous-binding\n",
        "a failed build must preserve the previous binding"
    );
    assert_eq!(
        fs::read(&generation_a).unwrap(),
        original_bytes,
        "a failed build must preserve published generation bytes"
    );
    assert_eq!(
        hash(&generation_a),
        digest,
        "published executable hash must remain immutable"
    );

    eprintln!("Process results: concurrent builds returned the same generation; invalid source returned a Cargo failure");
    eprintln!("Overlap and serialization timing: two concurrent calls completed in {elapsed:?} under one fs2 worktree lock");
    eprintln!("Immutable hash: {digest}; prior binding bytes unchanged after failure");
}

#[test]
fn real_engine_build_accepts_private_target_from_canonical_worktree() {
    let worktree = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("workspace root must canonicalize");
    let manifest = worktree.join("Cargo.toml");
    let private_target = worktree.join("target/gal-pipeline/cargo-target");
    let cargo_worktree = pipeline_execution::strip_windows_verbatim_prefix(&worktree);
    let cargo_manifest = pipeline_execution::strip_windows_verbatim_prefix(&manifest);
    let cargo_target = pipeline_execution::strip_windows_verbatim_prefix(&private_target);
    fs::create_dir_all(&private_target).unwrap();

    let output = std::process::Command::new("cargo")
        .current_dir(&cargo_worktree)
        .args(["build", "--manifest-path"])
        .arg(&cargo_manifest)
        .args(["-p", "gal-engine"])
        .env("CARGO_TARGET_DIR", &cargo_target)
        .output()
        .expect("cargo must start");
    assert!(
        output.status.success(),
        "cargo build -p gal-engine failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "builds the real gal binary into the worktree-private target; run with --ignored"]
fn real_worktree_build_reuses_the_published_generation_when_nothing_changed() {
    let worktree = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("workspace root must canonicalize");
    let manifest = worktree.join("Cargo.toml");
    let generations = worktree.join("target/gal-pipeline/bin");

    let first = pipeline_execution::build_private_generation(&worktree, &manifest)
        .expect("first private build must succeed");
    let count_after_first = fs::read_dir(&generations).unwrap().count();
    let second = pipeline_execution::build_private_generation(&worktree, &manifest)
        .expect("second private build must succeed");

    assert_eq!(
        first, second,
        "an unchanged tree must reuse the published generation"
    );
    assert_eq!(hash(&first), hash(&second));
    assert_eq!(
        fs::read_dir(&generations).unwrap().count(),
        count_after_first,
        "a repeated build must not publish another generation"
    );
}
