use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn source_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("plugins")
        .join("gal-core")
}

fn run_refresh(home: &Path, root: &Path, source: &Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command
        .args(["dev-apply", "--source"])
        .arg(source)
        .arg("--root")
        .arg(root);
    command.env("USERPROFILE", home).env("HOME", home);
    command.output().expect("run gal dev-apply")
}

fn run_refresh_without_source(home: &Path, root: &Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.args(["dev-apply", "--root"]).arg(root);
    command.env("USERPROFILE", home).env("HOME", home);
    command.output().expect("run gal dev-apply without source")
}

fn run_refresh_in(home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.arg("refresh").args(args);
    command
        .env("USERPROFILE", home)
        .env("HOME", home)
        .current_dir(cwd);
    command.output().expect("run gal refresh")
}

fn run_dev_apply_in(home: &Path, cwd: &Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.args(["dev-apply"]);
    command
        .env("USERPROFILE", home)
        .env("HOME", home)
        .current_dir(cwd);
    command.output().expect("run gal dev-apply")
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                pending.push(path);
            } else {
                entries.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn explicit_root_without_source_rejects_before_shared_home_writes() {
    let parent = TempDir::new().unwrap();
    let shared_home = parent.path().join("shared-home");
    let isolated_root = parent.path().join("isolated-root");
    let shared_gal = shared_home.join(".gal");
    fs::create_dir_all(shared_gal.join("embedded-src")).unwrap();
    fs::create_dir_all(&isolated_root).unwrap();
    fs::write(
        shared_gal.join("embedded-src/sentinel"),
        b"embedded fixture\n",
    )
    .unwrap();
    fs::write(shared_gal.join("marketplace.json"), b"shared fixture\n").unwrap();
    let before = snapshot_tree(&shared_gal);

    let result = run_refresh_without_source(&shared_home, &isolated_root);

    assert!(
        !result.status.success(),
        "--root without --source must reject"
    );
    assert_eq!(
        snapshot_tree(&shared_gal),
        before,
        "shared ~/.gal fixture changed"
    );
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("gal dev-apply: --source is required with --root"),
        "expected explicit source requirement, got: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn dev_apply_without_checkout_or_source_exits_1_and_writes_nothing() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = parent.path().join("empty");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let result = run_dev_apply_in(&home, &cwd);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("gal dev-apply:"));
    assert!(
        !home.join(".gal").exists(),
        "command wrote into isolated home"
    );
}

#[test]
fn dev_apply_in_gal_home_exits_1_and_writes_nothing() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = home.join(".gal/plugins/gal");
    fs::create_dir_all(cwd.join("skills")).unwrap();
    fs::create_dir_all(cwd.join("agents")).unwrap();
    fs::create_dir_all(cwd.join("commands")).unwrap();
    let before = snapshot_tree(&home);
    let result = run_dev_apply_in(&home, &cwd);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("gal dev-apply:"));
    assert_eq!(
        snapshot_tree(&home),
        before,
        "command changed isolated home"
    );
}

#[test]
fn explicit_root_refresh_isolated_and_rejects_shared_destinations() {
    let parent = TempDir::new().unwrap();
    let shared_home = parent.path().join("shared-home");
    let isolated_root = parent.path().join("isolated-root");
    fs::create_dir_all(&shared_home).unwrap();
    fs::create_dir_all(&isolated_root).unwrap();

    let shared_manifest = shared_home.join(".gal/plugins/.claude-plugin/marketplace.json");
    fs::create_dir_all(shared_manifest.parent().unwrap()).unwrap();
    fs::write(&shared_manifest, b"shared sentinel\n").unwrap();
    let before = fs::read(&shared_manifest).unwrap();

    let source = source_root();
    assert!(source.exists(), "source root missing: {}", source.display());
    let isolated = run_refresh(&shared_home, &isolated_root, &source);
    assert!(
        isolated.status.success(),
        "explicit-root refresh failed: {}",
        String::from_utf8_lossy(&isolated.stderr)
    );

    let isolated_manifest = isolated_root.join(".gal/plugins/.claude-plugin/marketplace.json");
    assert!(isolated_manifest.exists(), "isolated manifest missing");
    let after = fs::read(&shared_manifest).unwrap();
    assert_eq!(
        before, after,
        "shared manifest changed during isolated refresh"
    );

    let relative = run_refresh(&shared_home, Path::new("relative-root"), &source);
    let missing = run_refresh(&shared_home, &parent.path().join("missing-root"), &source);
    // Resolve the account profile independently of the compatibility variables
    // passed to each subprocess.
    let real_profile = operating_system_profile();
    let shared = run_refresh(&shared_home, &real_profile, &source);
    let protected_roots = [
        real_profile.join(".gal/bin"),
        real_profile.join(".gal/plugins/gal"),
        real_profile.join(".agents/skills"),
        real_profile.join(".claude/plugins/cache/gal"),
        real_profile.join(".gal/config"),
    ];
    let unrelated_env = parent.path().join("unrelated-env");
    let mut protected_results = Vec::new();
    for destination in &protected_roots {
        for env_home in [&isolated_root, &unrelated_env] {
            let result = run_refresh(env_home, destination, &source);
            assert!(
                !result.status.success(),
                "shared destination was accepted: {}",
                destination.display()
            );
            protected_results.push(result);
        }
    }
    let rejected = [&relative, &missing, &shared]
        .iter()
        .filter(|result| !result.status.success())
        .count();
    assert_eq!(rejected, 3, "all unsafe roots must be rejected");
    assert_eq!(protected_results.len(), 10);
    assert_eq!(fs::read(&shared_manifest).unwrap(), before);

    println!("shared manifest before: {:02x?}", before);
    println!("shared manifest after: {:02x?}", after);
    println!(
        "isolated manifest: {}",
        fs::read_to_string(isolated_manifest).unwrap()
    );
    println!(
        "rejection totals: unsafe={rejected}, shared_destinations={}, accepted=1",
        protected_results.len()
    );
}

#[test]
fn refresh_rejects_source_and_root_options() {
    for args in [
        vec!["--source", "checkout"],
        vec!["--source=checkout"],
        vec!["--root", "destination"],
        vec!["--root=destination"],
        vec!["--source"],
        vec!["--root"],
    ] {
        let parent = TempDir::new().unwrap();
        let home = parent.path().join("home");
        let cwd = parent.path().join("empty");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        let result = run_refresh_in(&home, &cwd, &args);
        assert_eq!(result.status.code(), Some(64), "args={args:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("is not supported"),
            "expected 'is not supported' in stderr for args={args:?}: {stderr}"
        );
        assert!(
            stderr.contains("gal dev-apply"),
            "expected 'gal dev-apply' in stderr for args={args:?}: {stderr}"
        );
        assert!(
            !home.join(".gal").exists(),
            "rejected option wrote to home: args={args:?}"
        );
    }
}

#[test]
fn refresh_in_checkout_prints_notice_and_uses_installed_source() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let fake_checkout = parent.path().join("fake-checkout");
    let gal_core = fake_checkout.join("plugins/gal-core");
    fs::create_dir_all(gal_core.join("skills")).unwrap();
    fs::create_dir_all(gal_core.join("agents")).unwrap();
    fs::create_dir_all(gal_core.join("commands")).unwrap();

    let result = run_refresh_in(&home, &fake_checkout, &[]);
    assert!(
        result.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("this directory is a GAL source checkout"),
        "notice missing: {stderr}"
    );
    assert!(
        stderr.contains("gal dev-apply"),
        "dev-apply missing from notice: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    let source_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("source:"))
        .expect("source line missing from stdout");
    let fake_checkout_str = fake_checkout.to_string_lossy();
    assert!(
        !source_line.contains(&*fake_checkout_str),
        "source line must not be under temp checkout: {source_line}"
    );
}

#[test]
fn refresh_outside_checkout_prints_no_notice() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = parent.path().join("empty");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let result = run_refresh_in(&home, &cwd, &[]);
    assert!(
        result.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!String::from_utf8_lossy(&result.stderr)
        .contains("this directory is a GAL source checkout"));
}

#[test]
fn refresh_in_gal_home_prints_no_notice() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = home.join(".gal/plugins/gal");
    fs::create_dir_all(cwd.join("skills")).unwrap();
    fs::create_dir_all(cwd.join("agents")).unwrap();
    fs::create_dir_all(cwd.join("commands")).unwrap();
    let result = run_refresh_in(&home, &cwd, &[]);
    assert!(
        result.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!String::from_utf8_lossy(&result.stderr)
        .contains("this directory is a GAL source checkout"));
}

#[test]
fn refresh_and_dev_apply_render_identical_canonical_roots() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("isolated-home");
    let cwd = parent.path().join("empty");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    let refresh = run_refresh_in(&home, &cwd, &[]);
    assert!(
        refresh.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&refresh.stderr)
    );

    let refresh_canonical = snapshot_tree(&home.join(".gal/plugins/gal"));
    let embedded_src = home.join(".gal/embedded-src");
    assert!(
        embedded_src.exists(),
        "embedded-src must exist after refresh in empty cwd"
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.args(["dev-apply", "--source"]).arg(&embedded_src);
    command.env("USERPROFILE", &home).env("HOME", &home);
    let dev = command.output().expect("run gal dev-apply");
    assert!(
        dev.status.success(),
        "dev-apply failed: {}",
        String::from_utf8_lossy(&dev.stderr)
    );

    let dev_canonical = snapshot_tree(&home.join(".gal/plugins/gal"));
    assert_eq!(refresh_canonical, dev_canonical);
}

#[cfg(windows)]
fn operating_system_profile() -> PathBuf {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    #[repr(C)]
    struct Guid {
        d1: u32,
        d2: u16,
        d3: u16,
        d4: [u8; 8],
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetKnownFolderPath(
            id: *const Guid,
            flags: u32,
            token: isize,
            path: *mut *mut u16,
        ) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoTaskMemFree(ptr: *mut std::ffi::c_void);
    }
    const PROFILE: Guid = Guid {
        d1: 0x5e6c858f,
        d2: 0x0e22,
        d3: 0x4760,
        d4: [0x9a, 0xfe, 0xea, 0x33, 0x17, 0xb6, 0x71, 0x73],
    };
    let mut ptr = std::ptr::null_mut();
    assert_eq!(unsafe { SHGetKnownFolderPath(&PROFILE, 0, 0, &mut ptr) }, 0);
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let path = OsString::from_wide(unsafe { std::slice::from_raw_parts(ptr, len) });
    unsafe {
        CoTaskMemFree(ptr.cast());
    }
    path.into()
}

#[cfg(not(windows))]
fn operating_system_profile() -> PathBuf {
    extern "C" {
        fn getuid() -> u32;
    }
    let uid = unsafe { getuid() };
    std::process::Command::new("getent")
        .args(["passwd", &uid.to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().split(':').nth(5).map(PathBuf::from))
        .unwrap()
}
