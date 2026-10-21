//! Integration test verifying that gal-engine doctor uses machine_config_path authority on Windows.

#[cfg(target_os = "windows")]
mod windows_tests {
    use gal_engine::doctor::{HealthCheck, SetupHealthCheck};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn doctor_setup_check_uses_machine_config_path_authority() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Override USERPROFILE to point to the temp directory
        unsafe {
            std::env::set_var("USERPROFILE", temp_path);
        }

        // Create <temp>/.gal/config/config.json as a directory so read_to_string fails
        let config_dir = temp_path.join(".gal").join("config").join("config.json");
        fs::create_dir_all(&config_dir).unwrap();

        let check = SetupHealthCheck {
            repo_root: temp_path.to_path_buf(),
        };

        let findings = check.check();

        let temp_path_str = temp_path.to_str().unwrap();
        let found = findings.iter().any(|f| f.message.contains(temp_path_str));

        assert!(
            found,
            "Expected SetupHealthCheck findings to name temp path {:?}, but findings were: {:?}",
            temp_path_str, findings
        );
    }
}
