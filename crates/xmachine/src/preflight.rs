//! preflight (T-008, R-05): remote-precondition health checks.
//!
//! Exposes SSH / zellij / remote-`gal` readiness as `base::HealthCheck` so
//! `gal doctor` aggregates them the same way as every other GAL surface. These
//! checks are **check-only**: GAL never configures SSH, never installs zellij, and
//! never scp's a `gal` binary — a hard boundary. To make that structural (and to
//! keep grading unit-testable), each check holds a pre-collected probe outcome;
//! the IO probe lives in the caller (the `cli` edge), and *this module contains no
//! provisioning command at all*. Missing preconditions fail loud with the fix
//! guidance the user must act on themselves.

use base::health::{DoctorFinding, HealthCheck};

use crate::ssh::ssh_invocation_args;

/// A harmless, side-effect-free reachability + remote-`gal` probe: run
/// `gal --version` on the remote over SSH. Building the args performs no IO.
pub fn remote_gal_version_probe_args(ssh_target: &str) -> Vec<String> {
    ssh_invocation_args(ssh_target, "gal --version")
}

/// Exact-match version compatibility (legacy preflight rejects on mismatch and
/// guides the user to update the remote `gal`; GAL never auto-updates it).
pub fn gal_version_compatible(required: &str, remote: &str) -> bool {
    required == remote
}

/// SSH reachability of a work node (probe result injected by the caller).
#[derive(Debug, Clone)]
pub struct SshReachableCheck {
    pub node: String,
    pub reachable: bool,
    pub detail: Option<String>,
}

impl HealthCheck for SshReachableCheck {
    fn name(&self) -> &str {
        "xmachine.ssh"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if self.reachable {
            return vec![];
        }
        let detail = self
            .detail
            .as_deref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        vec![DoctorFinding::error(
            format!("work node '{}' is not reachable over SSH{detail}", self.node),
            "configure passwordless key-based SSH to the work node yourself \
             (add your public key, verify BatchMode connect); GAL never configures SSH",
        )]
    }
}

/// zellij availability on a work node (probe result injected by the caller).
#[derive(Debug, Clone)]
pub struct ZellijInstalledCheck {
    pub node: String,
    pub installed: bool,
    /// Whether the `script` pty helper is also present (needed for detached zellij).
    pub script_helper_present: bool,
}

impl HealthCheck for ZellijInstalledCheck {
    fn name(&self) -> &str {
        "xmachine.zellij"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if self.installed && self.script_helper_present {
            return vec![];
        }
        vec![DoctorFinding::error(
            format!(
                "work node '{}' is missing the detached launcher (zellij + script)",
                self.node
            ),
            "install zellij plus the system 'script' utility on the work node, \
             or ensure 'nohup' is available; GAL never installs zellij",
        )]
    }
}

/// Remote `gal` compatibility on a work node (probe result injected by the caller).
#[derive(Debug, Clone)]
pub struct RemoteGalCheck {
    pub node: String,
    pub required_version: String,
    /// The remote `gal --version`, or `None` when `gal` was not found remotely.
    pub remote_version: Option<String>,
}

impl HealthCheck for RemoteGalCheck {
    fn name(&self) -> &str {
        "xmachine.remote-gal"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        match &self.remote_version {
            None => vec![DoctorFinding::error(
                format!("work node '{}' has no compatible remote `gal` binary", self.node),
                "install a compatible end-user `gal` release on the work node yourself; \
                 GAL never scp's or builds `gal` remotely",
            )],
            Some(remote) if !gal_version_compatible(&self.required_version, remote) => {
                vec![DoctorFinding::error(
                    format!(
                        "work node '{}' remote `gal` {remote} does not match required {}",
                        self.node, self.required_version
                    ),
                    "update the remote `gal` to the matching release yourself; \
                     GAL never auto-updates it",
                )]
            }
            Some(_) => vec![],
        }
    }
}

/// Build the full preflight check set for one work node, given pre-collected probe
/// results. The returned trait objects feed `gal doctor` aggregation.
pub fn node_health_checks(
    node: &str,
    ssh_reachable: bool,
    ssh_detail: Option<String>,
    zellij_installed: bool,
    script_helper_present: bool,
    required_gal: &str,
    remote_gal: Option<String>,
) -> Vec<Box<dyn HealthCheck>> {
    vec![
        Box::new(SshReachableCheck {
            node: node.to_string(),
            reachable: ssh_reachable,
            detail: ssh_detail,
        }),
        Box::new(ZellijInstalledCheck {
            node: node.to_string(),
            installed: zellij_installed,
            script_helper_present,
        }),
        Box::new(RemoteGalCheck {
            node: node.to_string(),
            required_version: required_gal.to_string(),
            remote_version: remote_gal,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::health::Severity;

    #[test]
    fn reachable_node_has_no_findings() {
        let c = SshReachableCheck {
            node: "mac-mini".into(),
            reachable: true,
            detail: None,
        };
        assert!(c.check().is_empty());
    }

    #[test]
    fn unreachable_ssh_is_error_with_user_owned_fix() {
        let c = SshReachableCheck {
            node: "mac-mini".into(),
            reachable: false,
            detail: Some("Permission denied (publickey)".into()),
        };
        let findings = c.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("not reachable over SSH"));
        assert!(findings[0].message.contains("publickey"));
        assert!(findings[0]
            .fix_hint
            .as_deref()
            .unwrap()
            .contains("GAL never configures SSH"));
    }

    #[test]
    fn zellij_present_only_when_both_zellij_and_script() {
        assert!(ZellijInstalledCheck {
            node: "n".into(),
            installed: true,
            script_helper_present: true,
        }
        .check()
        .is_empty());

        let missing = ZellijInstalledCheck {
            node: "n".into(),
            installed: true,
            script_helper_present: false,
        };
        assert_eq!(missing.check().len(), 1);
        assert!(missing.check()[0]
            .fix_hint
            .as_deref()
            .unwrap()
            .contains("GAL never installs zellij"));
    }

    #[test]
    fn remote_gal_missing_is_error() {
        let c = RemoteGalCheck {
            node: "n".into(),
            required_version: "gal 0.1.0".into(),
            remote_version: None,
        };
        assert_eq!(c.check().len(), 1);
        assert!(c.check()[0].message.contains("no compatible remote `gal`"));
    }

    #[test]
    fn remote_gal_version_mismatch_is_error_with_update_guidance() {
        let c = RemoteGalCheck {
            node: "n".into(),
            required_version: "gal 0.1.0".into(),
            remote_version: Some("gal 0.0.9".into()),
        };
        let f = c.check();
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("does not match required"));
        assert!(f[0].fix_hint.as_deref().unwrap().contains("never auto-updates"));
    }

    #[test]
    fn remote_gal_matching_version_passes() {
        let c = RemoteGalCheck {
            node: "n".into(),
            required_version: "gal 0.1.0".into(),
            remote_version: Some("gal 0.1.0".into()),
        };
        assert!(c.check().is_empty());
        assert!(gal_version_compatible("gal 0.1.0", "gal 0.1.0"));
        assert!(!gal_version_compatible("gal 0.1.0", "gal 0.0.9"));
    }

    #[test]
    fn version_probe_is_a_harmless_remote_read() {
        let args = remote_gal_version_probe_args("alice@mac-mini");
        assert_eq!(
            args,
            vec!["-o", "BatchMode=yes", "alice@mac-mini", "gal --version"]
        );
    }

    #[test]
    fn node_health_checks_cover_all_three_surfaces() {
        let checks = node_health_checks(
            "mac-mini",
            false,
            None,
            false,
            false,
            "gal 0.1.0",
            None,
        );
        let names: Vec<&str> = checks.iter().map(|c| c.name()).collect();
        assert_eq!(names, vec!["xmachine.ssh", "xmachine.zellij", "xmachine.remote-gal"]);
        // all three fail loud when nothing is ready
        let total: usize = checks.iter().map(|c| c.check().len()).sum();
        assert_eq!(total, 3);
    }
}
