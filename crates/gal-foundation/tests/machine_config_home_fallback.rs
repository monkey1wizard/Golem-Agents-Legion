use std::env;
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

use gal_foundation::paths::{gal_home, machine_config_path};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock_guard: MutexGuard<'static, ()>,
    orig_home: Option<OsString>,
    orig_userprofile: Option<OsString>,
}

impl EnvGuard {
    fn remove_home_vars() -> Self {
        let lock_guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let orig_home = env::var_os("HOME");
        let orig_userprofile = env::var_os("USERPROFILE");

        env::remove_var("HOME");
        env::remove_var("USERPROFILE");

        Self {
            _lock_guard: lock_guard,
            orig_home,
            orig_userprofile,
        }
    }
}

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
fn machine_config_path_falls_back_to_dirs_home_dir() {
    let _guard = EnvGuard::remove_home_vars();

    // R5 asymmetry lock: gal_home() must return None when environment variables are unset.
    assert_eq!(
        gal_home(),
        None,
        "gal_home() must return None when home env vars are unset"
    );

    let expected_home =
        dirs::home_dir().expect("dirs::home_dir() must resolve on executing machine");
    let expected_config_path = expected_home
        .join(".gal")
        .join("config")
        .join("config.json");

    // Machine config path fallback check. Pre-change returns None and panics with expected assertion message.
    let actual_config_path = machine_config_path().expect("machine-config-fallback-missing");

    assert_eq!(
        actual_config_path, expected_config_path,
        "machine-config-fallback-missing"
    );
}
