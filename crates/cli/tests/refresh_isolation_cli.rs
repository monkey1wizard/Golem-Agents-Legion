use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

mod common;
use common::copy_executable;

fn run_refresh(home: &Path, root: &Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.args(["refresh", "--root"]).arg(root);
    command.env("USERPROFILE", home).env("HOME", home);
    command.output().expect("run gal refresh")
}

fn run_refresh_in(home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    let bin = TempDir::new().unwrap();
    fs::copy(
        env!("CARGO_BIN_EXE_gal"),
        bin.path()
            .join(format!("gal{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.arg("refresh").args(args);
    command
        .env("USERPROFILE", home)
        .env("HOME", home)
        .env("PATH", bin.path())
        .env("GAL_REFRESH_PATH", bin.path())
        .env_remove("CARGO")
        .current_dir(cwd);
    command.output().expect("run gal refresh")
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
fn explicit_root_preserves_shared_home() {
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

    let result = run_refresh(&shared_home, &isolated_root);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        snapshot_tree(&shared_gal),
        before,
        "shared ~/.gal fixture changed"
    );
}

#[test]
fn explicit_root_uses_embedded_contract_from_binary() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let root = parent.path().join("root");
    let empty = parent.path().join("empty");
    let checkout = parent.path().join("checkout");
    let bin_dir = parent.path().join("bin");
    for dir in [&home, &root, &empty, &bin_dir] {
        fs::create_dir_all(dir).unwrap();
    }
    for dir in ["skills", "agents", "commands"] {
        fs::create_dir_all(checkout.join("plugins/gal-core").join(dir)).unwrap();
        fs::create_dir_all(bin_dir.join("plugins/gal-core").join(dir)).unwrap();
    }
    fs::write(
        bin_dir.join("plugins/gal-core/skills/fake-contract"),
        b"fake",
    )
    .unwrap();
    fs::write(home.join("sentinel"), b"unchanged").unwrap();
    let before = snapshot_tree(&home);
    let exe = bin_dir.join(format!("gal{}", std::env::consts::EXE_SUFFIX));
    copy_executable(Path::new(env!("CARGO_BIN_EXE_gal")), &exe);
    for cwd in [&checkout, &empty] {
        let output = Command::new(&exe)
            .args(["refresh", "--root"])
            .arg(&root)
            .env("USERPROFILE", &home)
            .env("HOME", &home)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("current_exe:"), "{stdout}");
        assert!(stdout.contains("deployed-source"), "{stdout}");
        assert!(!root
            .join(".gal/deployed-source/skills/fake-contract")
            .exists());
        assert!(!root.join(".gal/plugins/gal/skills/fake-contract").exists());
        assert_eq!(snapshot_tree(&home), before);
    }
}

#[test]
fn removed_dev_apply_prints_migration_hint_without_writes() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = parent.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    for args in [
        &["dev-apply"][..],
        &["dev-apply", "--source", "x", "--root", "y"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .env("USERPROFILE", &home)
            .env("HOME", &home)
            .current_dir(&cwd)
            .output()
            .expect("run gal dev-apply");
        assert_eq!(output.status.code(), Some(64));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&output.stderr).lines().collect::<Vec<_>>(),
            vec!["gal dev-apply was removed. From the GAL worktree, set PATH and GAL_REFRESH_PATH, then run: cargo run --locked --bin gal -- refresh [--root <dir>]"]);
        assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&cwd).unwrap().count(), 0);
    }
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

    let isolated = run_refresh(&shared_home, &isolated_root);
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

    let relative = run_refresh(&shared_home, Path::new("relative-root"));
    let missing = run_refresh(&shared_home, &parent.path().join("missing-root"));
    // Resolve the account profile independently of the compatibility variables
    // passed to each subprocess.
    let real_profile = operating_system_profile();
    let shared = run_refresh(&shared_home, &real_profile);
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
            let result = run_refresh(env_home, destination);
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
fn refresh_rejects_source_and_invalid_options() {
    for args in [
        vec!["--source", "checkout"],
        vec!["--source=checkout"],
        vec!["--root=destination"],
        vec!["--source"],
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
            stderr.contains("unsupported"),
            "expected unsupported option in stderr for args={args:?}: {stderr}"
        );
        assert!(
            !stderr.contains("gal dev-apply"),
            "unexpected dev-apply in stderr for args={args:?}: {stderr}"
        );
        assert!(
            !home.join(".gal").exists(),
            "rejected option wrote to home: args={args:?}"
        );
    }
}

#[test]
fn refresh_rejects_invalid_roots_without_writes() {
    let parent = TempDir::new().unwrap();
    let home = parent.path().join("home");
    let cwd = parent.path().join("empty");
    let missing = parent.path().join("missing");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    for root in [Path::new("relative"), missing.as_path()] {
        let result = run_refresh_in(&home, &cwd, &["--root", root.to_str().unwrap()]);
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("refresh root"), "{stderr}");
        assert!(!stderr.contains("dev-apply"), "{stderr}");
        assert!(snapshot_tree(&home).is_empty());
        assert!(snapshot_tree(&cwd).is_empty());
        assert!(!missing.exists());
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
        stderr.contains("cargo run --locked --bin gal -- refresh"),
        "development guidance missing from notice: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    let source_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("source:"))
        .expect("source line missing from stdout");
    let fake_checkout_str = fake_checkout.to_string_lossy();
    assert!(source_line.contains("deployed-source"), "{source_line}");
    assert!(stdout.contains("current_exe:"), "{stdout}");
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

#[test]
fn shared_refresh_rejects_different_path_binary_before_writing() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let root = temp.path().join("root");
    let same = temp.path().join("same");
    let different = temp.path().join("different");
    for dir in [&home, &root, &same, &different] {
        fs::create_dir(dir).unwrap();
    }
    let name = format!("gal{}", std::env::consts::EXE_SUFFIX);
    let candidate = different.join(&name);
    copy_executable(Path::new(env!("CARGO_BIN_EXE_gal")), &same.join(&name));
    fs::write(&candidate, b"different").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = |isolated: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
        cmd.arg("refresh")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PATH", &same)
            .env("GAL_REFRESH_PATH", &different)
            .env("CARGO", "fixture");
        if isolated {
            cmd.arg("--root").arg(&root);
        }
        cmd.output().unwrap()
    };
    let before = snapshot_tree(&home);
    let result = run(false);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(
        stderr.contains("current_exe:")
            && stderr.contains("GAL_REFRESH_PATH")
            && stderr.contains(&candidate.display().to_string()),
        "{stderr}"
    );
    assert_eq!(snapshot_tree(&home), before);
    let isolated = run(true);
    assert!(
        isolated.status.success(),
        "{}",
        String::from_utf8_lossy(&isolated.stderr)
    );
}

#[cfg(windows)]
#[test]
fn cargo_without_refresh_path_rejects_shared_but_allows_isolated_refresh() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let root = temp.path().join("root");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&root).unwrap();
    let run = |isolated: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
        cmd.arg("refresh")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PATH", &home)
            .env_remove("GAL_REFRESH_PATH")
            .env("CARGO", "fixture");
        if isolated {
            cmd.arg("--root").arg(&root);
        }
        cmd.output().unwrap()
    };
    let before = snapshot_tree(&home);
    let result = run(false);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("GAL_REFRESH_PATH"));
    assert_eq!(snapshot_tree(&home), before);
    let isolated = run(true);
    assert!(
        isolated.status.success(),
        "{}",
        String::from_utf8_lossy(&isolated.stderr)
    );
}
