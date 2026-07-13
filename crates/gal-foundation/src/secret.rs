//! Secret-detection regex — gal's foundation copy.
//!
//! gal keeps its own secret regex here so the workflow `gal doctor` manifest
//! scan (`gal-engine`'s doctor) can reach it without depending on any management
//! crate. A second copy lives alongside the `gal clean` filter
//! (`crates/cli/src/gal/git_filters.rs`); the two diverge freely
//! (decoupling > DRY) and neither imports the other.

use regex::Regex;

/// Returns a compiled regex that matches secret-like values in content.
///
/// Pattern: `(API_KEY|TOKEN|SECRET|PASSWORD|PAT)[:=] <non-whitespace-non-angle>+`
/// Covers common credential key suffixes including PAT (personal access token).
pub fn secret_re() -> Regex {
    Regex::new(r"(API_KEY|TOKEN|SECRET|PASSWORD|PAT)[[:space:]]*[:=][[:space:]]*[^<[:space:]]+")
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_secret_shapes() {
        let re = secret_re();
        assert!(re.is_match("API_KEY=sk-abc123"));
        assert!(re.is_match("TOKEN: ghp_realtokenvalue"));
        assert!(re.is_match("PASSWORD = hunter2"));
        assert!(re.is_match("PAT=github_pat_xxx"));
    }

    #[test]
    fn ignores_plain_words_and_placeholders() {
        let re = secret_re();
        assert!(!re.is_match("the token is rotated weekly"));
        assert!(!re.is_match("API_KEY=<your-key-here>"));
        assert!(!re.is_match("just some prose about a secret garden"));
    }
}
