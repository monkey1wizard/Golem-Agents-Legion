//! Integration test for routing home directory resolution.

#[cfg(windows)]
use std::env;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard};
#[cfg(windows)]
use tempfile::TempDir;

#[cfg(windows)]
use dispatch::routing::load_routing_default;

#[cfg(windows)]
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
struct EnvGuard {
    _lock_guard: MutexGuard<'static, ()>,
    orig_home: Option<OsString>,
    orig_userprofile: Option<OsString>,
}

#[cfg(windows)]
impl EnvGuard {
    fn set_userprofile(temp_path: &std::path::Path) -> Self {
        let lock_guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let orig_home = env::var_os("HOME");
        let orig_userprofile = env::var_os("USERPROFILE");

        env::remove_var("HOME");
        env::set_var("USERPROFILE", temp_path);

        Self {
            _lock_guard: lock_guard,
            orig_home,
            orig_userprofile,
        }
    }
}

#[cfg(windows)]
impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(val) = &self.orig_home {
            env::set_var("HOME", val);
        } else {
            env::remove_var("HOME");
        }
        if let Some(val) = &self.orig_userprofile {
            env::set_var("USERPROFILE", val);
        } else {
            env::remove_var("USERPROFILE");
        }
    }
}

#[test]
#[cfg(windows)]
fn routing_default_resolves_via_userprofile_override() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let config_dir = temp.path().join(".gal").join("config");
    fs::create_dir_all(&config_dir).expect("failed to create config dir");

    let config_file = config_dir.join("config.json");
    let sentinel_executor = "sentinel-executor-t05";
    let sentinel_model = "sentinel-model-t05-spec";

    let json_content = format!(
        r#"{{
            "executorRouting": {{
                "pipeline": {{
                    "CODER": {{
                        "executor": "{}",
                        "model": "{}"
                    }}
                }}
            }}
        }}"#,
        sentinel_executor, sentinel_model
    );
    fs::write(&config_file, json_content).expect("failed to write config.json");

    let _guard = EnvGuard::set_userprofile(temp.path());

    let table = load_routing_default();
    let coder_route = table
        .get("CODER")
        .expect("RoutingTable::get(\"CODER\") must yield entry from overridden USERPROFILE config");

    assert_eq!(
        coder_route.executor, sentinel_executor,
        "executor must match sentinel value"
    );
    assert_eq!(
        coder_route.model, sentinel_model,
        "model must match sentinel value"
    );
}

#[test]
#[cfg(not(windows))]
fn routing_default_resolves_via_userprofile_override_non_windows_stub() {
    // This non-red probe is scoped to Windows because dirs::home_dir() reads HOME first on Unix.
}
