//! zellij (T-006): remote session multiplexing command construction.
//!
//! Ports the verified zellij detach/poll/run pattern from
//! `Invoke-XmachinePipeline.{ps1,sh}` and `Invoke-XmachineLocalTask.sh`:
//! create a background session via `script`, poll `list-sessions` until it
//! appears, fail loud if it never does, then `run --close-on-exit`. Reattach and
//! teardown are also modeled. zellij itself is a user precondition (GAL preflights
//! it, never installs it — see `preflight`); this module only builds the commands.

/// Detached-launcher choice. zellij is preferred; nohup is the fallback when
/// zellij or the `script` pty helper is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launcher {
    Zellij,
    Nohup,
}

/// Pick the launcher (parity with the legacy `command -v zellij && command -v
/// script` probe; otherwise nohup).
pub fn select_launcher(has_zellij: bool, has_script: bool) -> Launcher {
    if has_zellij && has_script {
        Launcher::Zellij
    } else {
        Launcher::Nohup
    }
}

/// Attempts to poll `list-sessions` before declaring the session dead (legacy
/// loop of 10 with a 0.3s sleep).
pub const SESSION_WAIT_ATTEMPTS: u32 = 10;

/// Exit code emitted when the background session never materializes (legacy `exit 41`).
pub const MISSING_SESSION_EXIT_CODE: i32 = 41;

/// `script -q /dev/null zellij attach --create-background '<session>'` —
/// the `script` pty wrapper is what lets `--create-background` keep the session
/// detached.
pub fn create_background_command(session: &str) -> String {
    format!("script -q /dev/null zellij attach --create-background '{session}'")
}

/// `zellij list-sessions 2>/dev/null | grep -q -F '<session>'` — the readiness /
/// reconnect existence probe.
pub fn session_exists_command(session: &str) -> String {
    format!("zellij list-sessions 2>/dev/null | grep -q -F '{session}'")
}

/// `zellij --session '<session>' run --close-on-exit -- <inner>` — run the work
/// inside the session; `--close-on-exit` tears the pane down when the job ends.
pub fn run_in_session_command(session: &str, inner_command: &str) -> String {
    format!("zellij --session '{session}' run --close-on-exit -- {inner_command}")
}

/// `zellij delete-session --force '<session>'` — teardown after result collection.
pub fn delete_session_command(session: &str) -> String {
    format!("zellij delete-session --force '{session}'")
}

/// Human reconnect hint shown after dispatch.
pub fn attach_hint(session: &str) -> String {
    format!("zellij attach {session}              # detach with Ctrl-p, d")
}

/// Reconnect decision after a disconnect: if the session is still listed, reattach
/// to the live run; if it is gone, the run has ended (collect results instead).
pub fn should_reattach(session_present: bool) -> bool {
    session_present
}

/// The full remote launch script (parity with `Invoke-XmachinePipeline.sh`'s
/// inner SSH command): ensure output dir, background the session, poll for it,
/// fail loud with the missing-session exit code, then run the job inside it.
pub fn remote_launch_script(session: &str, output_dir: &str, inner_command: &str) -> String {
    let create = create_background_command(session);
    let exists = session_exists_command(session);
    let run = run_in_session_command(session, inner_command);
    let poll_seq = (1..=SESSION_WAIT_ATTEMPTS)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "set -euo pipefail; mkdir -p '{output_dir}'; {create} </dev/null >/dev/null 2>&1 & \
for _ in {poll_seq}; do if {exists}; then break; fi; sleep 0.3; done; \
if ! {exists}; then exit {code}; fi; {run} >/dev/null 2>&1",
        code = MISSING_SESSION_EXIT_CODE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_prefers_zellij_when_both_present() {
        assert_eq!(select_launcher(true, true), Launcher::Zellij);
        assert_eq!(select_launcher(true, false), Launcher::Nohup);
        assert_eq!(select_launcher(false, true), Launcher::Nohup);
        assert_eq!(select_launcher(false, false), Launcher::Nohup);
    }

    #[test]
    fn create_uses_script_pty_and_create_background() {
        assert_eq!(
            create_background_command("pipeline-20260611-abc123"),
            "script -q /dev/null zellij attach --create-background 'pipeline-20260611-abc123'"
        );
    }

    #[test]
    fn exists_probe_matches_legacy() {
        assert_eq!(
            session_exists_command("s1"),
            "zellij list-sessions 2>/dev/null | grep -q -F 's1'"
        );
    }

    #[test]
    fn run_in_session_closes_on_exit() {
        assert_eq!(
            run_in_session_command("s1", "bash /opt/runner.sh --run-id x"),
            "zellij --session 's1' run --close-on-exit -- bash /opt/runner.sh --run-id x"
        );
    }

    #[test]
    fn delete_session_is_forced() {
        assert_eq!(delete_session_command("s1"), "zellij delete-session --force 's1'");
    }

    #[test]
    fn reattach_only_when_session_is_present() {
        assert!(should_reattach(true));
        assert!(!should_reattach(false));
    }

    #[test]
    fn remote_launch_script_creates_polls_and_fails_loud() {
        let script = remote_launch_script("s1", "/tmp/out", "bash /opt/runner.sh");
        assert!(script.contains("mkdir -p '/tmp/out'"));
        assert!(script.contains("--create-background 's1'"));
        // polls then fails with exit 41 if the session never appears
        assert!(script.contains("if ! zellij list-sessions 2>/dev/null | grep -q -F 's1'; then exit 41;"));
        assert!(script.contains("zellij --session 's1' run --close-on-exit -- bash /opt/runner.sh"));
        // 10 poll iterations
        assert!(script.contains("for _ in 1 2 3 4 5 6 7 8 9 10; do"));
    }
}
