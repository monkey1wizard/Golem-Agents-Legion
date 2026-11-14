//! Shared helpers for `gal-cli` integration tests.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

fn is_worktrees_component(component: &Component<'_>) -> bool {
    ["worktrees", ".worktrees"].iter().any(|name| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
    })
}

/// Returns the `gal` binary path for tests to spawn.
///
/// A binary built inside a `worktrees` or `.worktrees` directory is copied to
/// `<repo root>/target/gal-test-bin-<pid>/`, so the spawned binary runs outside
/// any worktree. The repo root is the path above the first worktrees component,
/// minus a trailing `.claude`. The copy stays under the repository so a
/// repository-scoped antivirus exclusion still covers it.
#[allow(dead_code)]
pub fn test_gal_exe() -> &'static Path {
    static TEST_EXE: OnceLock<PathBuf> = OnceLock::new();
    TEST_EXE.get_or_init(|| {
        let src = PathBuf::from(env!("CARGO_BIN_EXE_gal"));
        let components: Vec<Component<'_>> = src.components().collect();
        let Some(index) = components.iter().position(is_worktrees_component) else {
            return src;
        };
        let mut root: PathBuf = components[..index].iter().collect();
        if root
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(".claude"))
        {
            root.pop();
        }
        let dir = root
            .join("target")
            .join(format!("gal-test-bin-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let dst = dir.join(src.file_name().unwrap());
        let _ = std::fs::copy(&src, &dst);
        dst
    })
}

/// Copies an executable and returns once it can be executed.
///
/// On Linux, a child forked by a parallel test can briefly inherit the write
/// handle of the copy, so exec fails with ETXTBSY until that child has exec'd.
/// This runs `--version` until exec no longer reports a busy file.
#[allow(dead_code)]
pub fn copy_executable(src: &Path, dst: &Path) {
    std::fs::copy(src, dst).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match std::process::Command::new(dst).arg("--version").output() {
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            result => {
                result.unwrap();
                return;
            }
        }
    }
}
