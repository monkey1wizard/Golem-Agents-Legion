//! gal-engine: shared primitives for the `gal` CLI.
//!
//! Provides configuration parsing (Rust owns config truth), command enum,
//! exit-code classification, and path resolution.

pub mod doctor;
pub mod embedded;
pub mod git_filter;
pub mod release;
pub mod render;
/// Freshness checker for `docs/i18n`, kept deliberately caller-less.
///
/// The copy this workspace's binary runs is `crates/cli/src/gal/translation.rs`.
/// This second copy is parked for the sibling product, so that each product
/// resolves its own documentation tree rather than sharing one implementation
/// across repositories. Decoupling was chosen over DRY when the two products
/// split, recorded in commit `db74c50b`. Its consumer lives in the sibling
/// repository, so no search confined to this workspace reaches it. Do not
/// remove it as dead code.
///
/// The two copies are kept byte-identical on purpose, so the module's own
/// header still names the `gal` command. Mirror every change to the copy above
/// into this one, as commit `edc0b6de` did. On port, repoint that copy's
/// `TERMINOLOGY_SOURCE` at the sibling product's own terminology authority.
pub mod translation;

#[cfg(test)]
pub(crate) mod test_support {
    /// Serializes tests that read or modify process-global home environment variables.
    pub(crate) static HOME_ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

// Foundation modules live in the `base` crate; re-exported here so existing
// `crate::config` / `gal_engine::config` consumer paths keep resolving.
pub use gal_foundation::{config, paths};

/// Known subcommands of the `gal` CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommandKind {
    Update,
    Doctor,
    /// `gal dispatch ...` — Rust CLI wrapper over the dispatch entry contract.
    Dispatch,
    /// `gal pipeline ...` — Rust CLI wrapper over the pipeline entry contract.
    Pipeline,
    /// `gal commit-msg <msg-file>` — git commit-msg hook (optional).
    CommitMsg,
    /// `gal clean` / `gal smudge` — git filter content transforms.
    Clean,
    Smudge,
    /// `gal init [--blank]` — initialize GAL in the current repository.
    Init,
    /// `gal render-adapters` — re-render AGENTS.md and CLAUDE.md from .dev/project.md.
    RenderAdapters,
    /// `gal translation-freshness` — report translation freshness across docs/i18n.
    TranslationFreshness,
    DispatchScript,
    /// `gal naming-gate [--staged] [paths...]` — scan durable surfaces for
    /// plan-task ID provenance + retired terms (advisory; see `docs/glossary.md`).
    NamingGate,
    /// `gal finalize-check <plan-or-prompt> --receipt <path>` — finalize-internal
    /// deterministic zero-trust precondition checks; emits a machine receipt and a
    /// nonzero exit when any check fails. Not a public `/gal` command.
    FinalizeCheck,
    /// `gal pipeline-log append --task <t> --phase <p> --kind <k> --level <l> --msg <m>`
    /// — pipeline-internal: append one structured loop-log event to the machine-local
    /// loop-log (the in-process append path for `gal pipeline`). Not a public `/gal` command.
    PipelineLog,
    /// `gal pipeline-clean <plan-or-prompt>` — remove one closed plan runtime directory.
    /// Pipeline-internal; not a public `/gal` command.
    PipelineClean,
    /// `gal release --version <v> [--output-dir <dir>] [--winget] [--homebrew] [--asset <f>...]`
    /// — generate release artifacts (checksums.txt, artifact-manifest.json, winget/homebrew
    /// manifests). Release-internal binary subcommand; NOT on the public `/gal` surface.
    Release,
    /// `gal release-notes [<from>..<to>]` — draft a deterministic CHANGELOG section from a
    /// commit range (default: highest HEAD-reachable stable `vX.Y.Z` tag .. HEAD).
    /// Release-internal binary subcommand; NOT on the public `/gal` surface.
    ReleaseNotes,
    /// `gal refresh` — idempotent machine-level rebuild of the canonical plugin root,
    /// parent Claude marketplace manifest, and per-runtime projections from source.
    /// Doubles as the dev restore/rebuild primitive for derived content.
    /// Internal binary subcommand; NOT on the public `/gal` surface.
    Refresh,
    /// `gal pipeline-converge-check <prompt> --task <t> [--receipt <p>]`
    /// — pipeline-internal 2g closeout: three-surface checkbox agreement + commit exists +
    /// `Current Task:` cleared. Receipt is the sole pass-basis. Not a public `/gal` command.
    PipelineConvergeCheck,
    /// `gal pipeline-handback-check <prompt> [--stop-at <task>] [--receipt <path>]`
    /// — pipeline-internal final authorization and continuation classifier. Not a public
    /// `/gal` command.
    PipelineHandbackCheck,
    /// `gal boundary-check <prompt> --task <t> [--receipt <p>]`
    /// — pipeline-internal 2c pre-commit: `git diff --name-only` + untracked vs the task's
    /// `## Affected Files` allowlist. Missing/empty allowlist → `NotRun` (non-zero), never
    /// allow-all. Not a public `/gal` command.
    BoundaryCheck,
    /// `gal pipeline-preflight <prompt> [--receipt <p>]`
    /// — pipeline-internal Step 1: prereq check + resume cursor + wrong-plan guard.
    /// Not a public `/gal` command.
    PipelinePreflight,
    /// `gal planning-check <source-plan> [--receipt <p>]`
    /// — planning-internal handoff gate: sections + OQ=None + ARCH_REVIEW clear + naming-gate
    /// + planLanguage consistency. Not a public `/gal` command.
    PlanningCheck,
    /// `gal prompt-check <prompt> [--receipt <p>]`
    /// — planning-internal compressed prompt anchor validator. Not a public `/gal` command.
    PromptCheck,
    /// `gal refining-check <source-plan> [--receipt <p>]`
    /// — planning-internal refined-plan structure gate. Not a public `/gal` command.
    RefiningCheck,
    /// `gal planning-stamp <localized-source>` (default) or
    /// `gal planning-stamp --equivalence <prompt> --receipt <p>`
    /// — planning-internal deterministic hash/receipt write-side: stamps the
    /// localized metadata hashes, or writes the tracked equivalence receipt.
    /// Not a public `/gal` command (peer of planning-check / prompt-check).
    PlanningStamp,
    /// `gal restore` — revert GAL source repo to last `gal-last-good` marker.
    /// Creates a backup branch then restores worktree to the marker commit.
    /// Internal binary subcommand; NOT on the public `/gal` surface.
    Restore,
    /// `gal marketplace-snapshot --source <dir> --out <dir>` — render the public
    /// marketplace plugin tree (core-only, no projection, no machine-local content)
    /// to an explicit output dir + write the marketplace catalog. Publisher-internal
    /// binary subcommand; NOT on the public `/gal` surface.
    MarketplaceSnapshot,
    /// `gal state-merge` — finalize-internal deterministic resolver for
    /// `.dev/state.md` merge conflicts whose conflicted path set is exactly
    /// that one file: row-keyed three-way merge, fail-closed on ambiguity.
    /// Not a public `/gal` command (peer of `finalize-check`).
    StateMerge,
    /// `gal research ...` — research dispatch lane entry point.
    Research,
    /// `gal pipeline-host-hook` — internal Codex manifest host-hook command.
    /// Not a public `/gal` command.
    PipelineHostHook,
}

impl CommandKind {
    /// Every known subcommand, in help/display order.
    pub const ALL: [CommandKind; 31] = [
        CommandKind::Update,
        CommandKind::Doctor,
        CommandKind::Dispatch,
        CommandKind::Pipeline,
        CommandKind::CommitMsg,
        CommandKind::Clean,
        CommandKind::Smudge,
        CommandKind::Init,
        CommandKind::RenderAdapters,
        CommandKind::TranslationFreshness,
        CommandKind::DispatchScript,
        CommandKind::NamingGate,
        CommandKind::FinalizeCheck,
        CommandKind::PipelineLog,
        CommandKind::PipelineClean,
        CommandKind::Release,
        CommandKind::ReleaseNotes,
        CommandKind::Refresh,
        CommandKind::PipelineConvergeCheck,
        CommandKind::PipelineHandbackCheck,
        CommandKind::BoundaryCheck,
        CommandKind::PipelinePreflight,
        CommandKind::PlanningCheck,
        CommandKind::PromptCheck,
        CommandKind::RefiningCheck,
        CommandKind::PlanningStamp,
        CommandKind::Restore,
        CommandKind::MarketplaceSnapshot,
        CommandKind::StateMerge,
        CommandKind::Research,
        CommandKind::PipelineHostHook,
    ];

    /// The canonical CLI spelling of this subcommand.
    pub fn as_str(self) -> &'static str {
        match self {
            CommandKind::Update => "update",
            CommandKind::Doctor => "doctor",
            CommandKind::Dispatch => "dispatch",
            CommandKind::Pipeline => "pipeline",
            CommandKind::CommitMsg => "commit-msg",
            CommandKind::Clean => "clean",
            CommandKind::Smudge => "smudge",
            CommandKind::Init => "init",
            CommandKind::RenderAdapters => "render-adapters",
            CommandKind::TranslationFreshness => "translation-freshness",
            CommandKind::DispatchScript => "dispatch-script",
            CommandKind::NamingGate => "naming-gate",
            CommandKind::FinalizeCheck => "finalize-check",
            CommandKind::PipelineLog => "pipeline-log",
            CommandKind::PipelineClean => "pipeline-clean",
            CommandKind::Release => "release",
            CommandKind::ReleaseNotes => "release-notes",
            CommandKind::Refresh => "refresh",
            CommandKind::PipelineConvergeCheck => "pipeline-converge-check",
            CommandKind::PipelineHandbackCheck => "pipeline-handback-check",
            CommandKind::BoundaryCheck => "boundary-check",
            CommandKind::PipelinePreflight => "pipeline-preflight",
            CommandKind::PlanningCheck => "planning-check",
            CommandKind::PromptCheck => "prompt-check",
            CommandKind::RefiningCheck => "refining-check",
            CommandKind::PlanningStamp => "planning-stamp",
            CommandKind::Restore => "restore",
            CommandKind::MarketplaceSnapshot => "marketplace-snapshot",
            CommandKind::StateMerge => "state-merge",
            CommandKind::Research => "research",
            CommandKind::PipelineHostHook => "pipeline-host-hook",
        }
    }

    /// Parse a token into a known subcommand, or `None` if unrecognized.
    pub fn parse(input: &str) -> Option<CommandKind> {
        match input.trim().to_ascii_lowercase().as_str() {
            "update" => Some(CommandKind::Update),
            "doctor" => Some(CommandKind::Doctor),
            "dispatch" => Some(CommandKind::Dispatch),
            "pipeline" => Some(CommandKind::Pipeline),
            "commit-msg" => Some(CommandKind::CommitMsg),
            "clean" => Some(CommandKind::Clean),
            "smudge" => Some(CommandKind::Smudge),
            "init" => Some(CommandKind::Init),
            "render-adapters" => Some(CommandKind::RenderAdapters),
            "translation-freshness" => Some(CommandKind::TranslationFreshness),
            "dispatch-script" => Some(CommandKind::DispatchScript),
            "naming-gate" => Some(CommandKind::NamingGate),
            "finalize-check" => Some(CommandKind::FinalizeCheck),
            "pipeline-log" => Some(CommandKind::PipelineLog),
            "pipeline-clean" => Some(CommandKind::PipelineClean),
            "release" => Some(CommandKind::Release),
            "release-notes" => Some(CommandKind::ReleaseNotes),
            "refresh" => Some(CommandKind::Refresh),
            "pipeline-converge-check" => Some(CommandKind::PipelineConvergeCheck),
            "pipeline-handback-check" => Some(CommandKind::PipelineHandbackCheck),
            "boundary-check" => Some(CommandKind::BoundaryCheck),
            "pipeline-preflight" => Some(CommandKind::PipelinePreflight),
            "planning-check" => Some(CommandKind::PlanningCheck),
            "prompt-check" => Some(CommandKind::PromptCheck),
            "refining-check" => Some(CommandKind::RefiningCheck),
            "planning-stamp" => Some(CommandKind::PlanningStamp),
            "restore" => Some(CommandKind::Restore),
            "marketplace-snapshot" => Some(CommandKind::MarketplaceSnapshot),
            "state-merge" => Some(CommandKind::StateMerge),
            "research" => Some(CommandKind::Research),
            "pipeline-host-hook" => Some(CommandKind::PipelineHostHook),
            _ => None,
        }
    }
}

/// Process exit-code classification for the CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExitCode {
    /// Completed successfully (and doctor: no errors, warnings only).
    Success = 0,
    /// Operation failed (install error, render error, uninstall error, doctor errors).
    Error = 1,
    /// Subcommand recognized but not yet wired to the legacy engine (Phase 1).
    NotWired = 2,
    /// Bad usage: unknown command or missing command.
    Usage = 64,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        std::process::ExitCode::from(code as u8)
    }
}

/// The action the CLI should take for a given argv (binary name already stripped).
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Version,
    Help,
    /// A known subcommand dispatched by the CLI binary rather than handled in-engine.
    NotWired(CommandKind),
    UnknownCommand(String),
    MissingCommand,
}

/// Classify raw CLI arguments (binary name already stripped).
///
/// Pure: never touches the filesystem or machine config.
pub fn classify_args(args: &[String]) -> Action {
    let first = match args.first() {
        Some(arg) => arg.as_str(),
        None => return Action::MissingCommand,
    };

    match first {
        "--version" | "-V" | "version" => Action::Version,
        "--help" | "-h" | "help" => Action::Help,
        other => match CommandKind::parse(other) {
            Some(cmd) => Action::NotWired(cmd),
            None => Action::UnknownCommand(other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_count_matches_variant_count() {
        assert_eq!(CommandKind::ALL.len(), 31);
        assert_eq!(CommandKind::StateMerge.as_str(), "state-merge");
        assert_eq!(
            CommandKind::parse("state-merge"),
            Some(CommandKind::StateMerge)
        );
        assert_eq!(CommandKind::PlanningStamp.as_str(), "planning-stamp");
        assert_eq!(
            CommandKind::parse("planning-stamp"),
            Some(CommandKind::PlanningStamp)
        );
        assert_eq!(CommandKind::Refresh.as_str(), "refresh");
        assert_eq!(
            CommandKind::PipelineConvergeCheck.as_str(),
            "pipeline-converge-check"
        );
        assert_eq!(CommandKind::BoundaryCheck.as_str(), "boundary-check");
        assert_eq!(
            CommandKind::PipelinePreflight.as_str(),
            "pipeline-preflight"
        );
        assert_eq!(CommandKind::PlanningCheck.as_str(), "planning-check");
        assert_eq!(CommandKind::PromptCheck.as_str(), "prompt-check");
        assert_eq!(CommandKind::RefiningCheck.as_str(), "refining-check");
    }

    #[test]
    fn research_is_a_registered_command_kind() {
        let action = classify_args(&["research".to_string()]);
        let cmd = match action {
            Action::NotWired(cmd) => cmd,
            _ => panic!("gal research is a registered command kind"),
        };
        assert_eq!(
            cmd.as_str(),
            "research",
            "gal research is a registered command kind"
        );
        assert!(
            CommandKind::ALL.iter().any(|c| c.as_str() == "research"),
            "gal research is a registered command kind"
        );
    }

    #[test]
    fn parses_every_known_command() {
        assert_eq!(CommandKind::parse("update"), Some(CommandKind::Update));
        assert_eq!(CommandKind::parse("doctor"), Some(CommandKind::Doctor));
        assert_eq!(CommandKind::parse("dispatch"), Some(CommandKind::Dispatch));
        assert_eq!(CommandKind::parse("pipeline"), Some(CommandKind::Pipeline));
        assert_eq!(CommandKind::parse("xmachine"), None);
        assert_eq!(CommandKind::parse("mcp"), None);
        assert_eq!(CommandKind::parse("uninstall"), None);
        assert_eq!(CommandKind::parse("clean"), Some(CommandKind::Clean));
        assert_eq!(CommandKind::parse("smudge"), Some(CommandKind::Smudge));
        assert_eq!(CommandKind::parse("init"), Some(CommandKind::Init));
        assert_eq!(
            CommandKind::parse("translation-freshness"),
            Some(CommandKind::TranslationFreshness)
        );
        assert_eq!(
            CommandKind::parse("dispatch-script"),
            Some(CommandKind::DispatchScript)
        );
        assert_eq!(
            CommandKind::parse("naming-gate"),
            Some(CommandKind::NamingGate)
        );
        assert_eq!(
            CommandKind::parse("finalize-check"),
            Some(CommandKind::FinalizeCheck)
        );
        assert_eq!(CommandKind::parse("refresh"), Some(CommandKind::Refresh));
        assert_eq!(
            CommandKind::parse("pipeline-converge-check"),
            Some(CommandKind::PipelineConvergeCheck)
        );
        assert_eq!(
            CommandKind::parse("boundary-check"),
            Some(CommandKind::BoundaryCheck)
        );
        assert_eq!(
            CommandKind::parse("pipeline-preflight"),
            Some(CommandKind::PipelinePreflight)
        );
        assert_eq!(
            CommandKind::parse("planning-check"),
            Some(CommandKind::PlanningCheck)
        );
        assert_eq!(
            CommandKind::parse("prompt-check"),
            Some(CommandKind::PromptCheck)
        );
        assert_eq!(
            CommandKind::parse("refining-check"),
            Some(CommandKind::RefiningCheck)
        );
        assert_eq!(CommandKind::parse(" DOCTOR "), Some(CommandKind::Doctor));
        assert_eq!(CommandKind::parse("restore"), Some(CommandKind::Restore));
        assert_eq!(CommandKind::parse("frobnicate"), None);
    }

    #[test]
    fn release_notes_command_round_trips() {
        assert_eq!(
            CommandKind::parse("release-notes"),
            Some(CommandKind::ReleaseNotes)
        );
        assert_eq!(CommandKind::ReleaseNotes.as_str(), "release-notes");
        assert!(CommandKind::ALL.contains(&CommandKind::ReleaseNotes));
    }

    #[test]
    fn pipeline_handback_check_command_kind_round_trips() {
        assert_eq!(
            CommandKind::parse("pipeline-handback-check"),
            Some(CommandKind::PipelineHandbackCheck)
        );
        assert_eq!(
            CommandKind::PipelineHandbackCheck.as_str(),
            "pipeline-handback-check"
        );
        assert_eq!(
            CommandKind::ALL
                .iter()
                .filter(|kind| **kind == CommandKind::PipelineHandbackCheck)
                .count(),
            1
        );
    }

    #[test]
    fn pipeline_host_hook_command_kind_round_trips() {
        assert_eq!(
            CommandKind::parse("pipeline-host-hook"),
            Some(CommandKind::PipelineHostHook)
        );
        assert_eq!(
            CommandKind::parse(" PIPELINE-HOST-HOOK "),
            Some(CommandKind::PipelineHostHook)
        );
        assert_eq!(CommandKind::PipelineHostHook.as_str(), "pipeline-host-hook");
        assert!(CommandKind::ALL.contains(&CommandKind::PipelineHostHook));
    }

    #[test]
    fn classifies_version_and_help() {
        assert_eq!(classify_args(&["--version".to_string()]), Action::Version);
        assert_eq!(classify_args(&["-V".to_string()]), Action::Version);
        assert_eq!(classify_args(&["--help".to_string()]), Action::Help);
        assert_eq!(classify_args(&[]), Action::MissingCommand);
    }

    #[test]
    fn classifies_known_and_unknown_subcommands() {
        assert_eq!(
            classify_args(&["doctor".to_string()]),
            Action::NotWired(CommandKind::Doctor)
        );
        assert_eq!(
            classify_args(&["frobnicate".to_string()]),
            Action::UnknownCommand("frobnicate".to_string())
        );
    }

    #[test]
    fn exit_codes_map_to_process_codes() {
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::NotWired as u8, 2);
        assert_eq!(ExitCode::Usage as u8, 64);
    }
}
