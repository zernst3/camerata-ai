//! Profile: a user's saved selection state.
//!
//! A profile records WHAT the user picked from the canonical library (by id)
//! plus any custom content the user authored (rules, domains, alternatives).
//! Canonical rule content is NOT embedded; on load, the current library is
//! consulted by id. This keeps profiles resilient to canonical-rule updates:
//! when a canonical rule's summary or why evolves, profiles automatically pick
//! up the new content because they only stored the id.
//!
//! Custom content is the inverse: the user is the source of truth, so it is
//! stored in full.

use crate::emit::CustomRule;
use crate::principle::Principle;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Profile schema version. Bumped to 2 when `chosen` changed from mapping a
/// rule id to the full ALTERNATIVE TEXT (v0.1) to mapping a rule id to the
/// chosen OPTION ID (decision-first schema). `migrate_chosen_to_option_ids`
/// best-effort upgrades a v1 `chosen` map against the current library.
pub const PROFILE_VERSION: u32 = 2;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub selected_ids: Vec<String>,
    /// Domains the user has active at save time. Restored on load so the
    /// per-rule selections re-anchor to a consistent domain context.
    /// `#[serde(default)]` so older profiles without this field still
    /// deserialize (interpreted as no domains active explicitly; the load
    /// flow seeds DEFAULT_SELECTED_DOMAINS as a backstop).
    #[serde(default)]
    pub selected_domains: Vec<String>,
    pub chosen: HashMap<String, String>,
    pub custom_alternatives: HashMap<String, Vec<String>>,
    pub custom_rules: Vec<CustomRule>,
    pub custom_domains: Vec<String>,
    pub out_dir: String,
    pub repos: Vec<String>,
    pub domain_repos: HashMap<String, Vec<String>>,
}

impl Profile {
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating parent dir for {}", path.display()))?;
        }
        let json = serde_json::to_string_pretty(self).context("serializing profile to JSON")?;
        std::fs::write(path, json)
            .with_context(|| format!("writing profile to {}", path.display()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading profile from {}", path.display()))?;
        let profile: Self = serde_json::from_str(&text)
            .with_context(|| format!("parsing profile JSON from {}", path.display()))?;
        Ok(profile)
    }

    /// Return the subset of selected_ids that no longer exist in the current
    /// library. Used to surface a soft warning when loading a profile against
    /// a newer library version that has renamed or removed rules.
    pub fn missing_ids<'a>(&self, library_ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let lib: HashSet<&str> = library_ids.into_iter().collect();
        self.selected_ids
            .iter()
            .filter(|id| !lib.contains(id.as_str()))
            .cloned()
            .collect()
    }

    /// Best-effort upgrade of a legacy v1 `chosen` map (rule id -> full
    /// alternative TEXT) to the decision-first form (rule id -> option id),
    /// matched against the current library. For each entry whose value is not
    /// already an option id of the rule, this text-matches the value against
    /// the rule's option directives (exact, then trimmed) and replaces it with
    /// the matching option id. Entries that match nothing are left untouched so
    /// no selection is silently dropped; the version is bumped regardless.
    ///
    /// Returns the rule ids whose `chosen` value could not be resolved to an
    /// option id, so the caller can surface a soft warning.
    pub fn migrate_chosen_to_option_ids(&mut self, library: &[Principle]) -> Vec<String> {
        let by_id: HashMap<&str, &Principle> = library.iter().map(|p| (p.id.as_str(), p)).collect();
        let mut unresolved = Vec::new();
        let mut upgraded: HashMap<String, String> = HashMap::new();
        for (rule_id, value) in &self.chosen {
            let Some(p) = by_id.get(rule_id.as_str()) else {
                // Rule no longer exists; keep the value verbatim, surface later.
                upgraded.insert(rule_id.clone(), value.clone());
                unresolved.push(rule_id.clone());
                continue;
            };
            // Already an option id? Nothing to do.
            if p.option(value).is_some() {
                upgraded.insert(rule_id.clone(), value.clone());
                continue;
            }
            // Legacy: value is full directive/alternative text. Match it to an
            // option by directive text (exact, then trimmed).
            let matched = p
                .options
                .iter()
                .find(|o| o.directive == *value)
                .or_else(|| {
                    p.options
                        .iter()
                        .find(|o| o.directive.trim() == value.trim())
                });
            match matched {
                Some(o) => {
                    upgraded.insert(rule_id.clone(), o.id.clone());
                }
                None => {
                    upgraded.insert(rule_id.clone(), value.clone());
                    unresolved.push(rule_id.clone());
                }
            }
        }
        self.chosen = upgraded;
        self.version = PROFILE_VERSION;
        unresolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_profile() -> Profile {
        let mut chosen = HashMap::new();
        chosen.insert(
            "CHOICE-RULE-1".to_string(),
            "the alternative text".to_string(),
        );
        let mut domain_repos = HashMap::new();
        domain_repos.insert("rust".to_string(), vec!["/repos/rust-repo".to_string()]);
        Profile {
            version: PROFILE_VERSION,
            selected_ids: vec!["UNIV-RULE-1".to_string(), "RUST-DOMAIN-4".to_string()],
            selected_domains: vec!["*".to_string(), "rust".to_string()],
            chosen,
            custom_alternatives: HashMap::new(),
            custom_rules: Vec::new(),
            custom_domains: Vec::new(),
            out_dir: "./out".to_string(),
            repos: vec!["/repos/default".to_string()],
            domain_repos,
        }
    }

    #[test]
    fn round_trip_through_disk_preserves_fields() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("profile.json");
        let original = sample_profile();
        original.save(&path).expect("save");

        let loaded = Profile::load(&path).expect("load");
        assert_eq!(loaded.version, PROFILE_VERSION);
        assert_eq!(loaded.selected_ids, original.selected_ids);
        assert_eq!(loaded.selected_domains, original.selected_domains);
        assert_eq!(
            loaded.chosen.get("CHOICE-RULE-1").map(String::as_str),
            Some("the alternative text"),
        );
        assert_eq!(loaded.out_dir, original.out_dir);
        assert_eq!(loaded.repos, original.repos);
        assert_eq!(
            loaded.domain_repos.get("rust").map(|v| v.as_slice()),
            Some(["/repos/rust-repo".to_string()].as_slice()),
        );
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let dir = tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("profile.json");
        Profile::default().save(&path).expect("save");
        assert!(
            path.exists(),
            "profile.json was created under nested parents"
        );
    }

    #[test]
    fn missing_ids_returns_ids_absent_from_library() {
        let p = sample_profile();
        let library = ["UNIV-RULE-1", "OTHER-RULE-1"];
        let missing = p.missing_ids(library);
        assert_eq!(missing, vec!["RUST-DOMAIN-4".to_string()]);
    }

    #[test]
    fn missing_ids_returns_empty_when_library_covers_all() {
        let p = sample_profile();
        let library = ["UNIV-RULE-1", "RUST-DOMAIN-4"];
        let missing = p.missing_ids(library);
        assert!(missing.is_empty());
    }

    #[test]
    fn missing_ids_returns_all_when_library_is_empty() {
        let p = sample_profile();
        let missing = p.missing_ids(std::iter::empty::<&str>());
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&"UNIV-RULE-1".to_string()));
        assert!(missing.contains(&"RUST-DOMAIN-4".to_string()));
    }

    fn library_for_migration() -> Vec<Principle> {
        let toml_text = r#"
id = "CHOICE-RULE-1"
title = "t"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
default = "primary"
why = "w"

[[option]]
id = "primary"
label = "primary"
directive = "the primary directive"
why = "w"

[[option]]
id = "alt"
label = "alt"
directive = "the alternative text"
why = "w"
"#;
        vec![toml::from_str(toml_text).expect("parses")]
    }

    #[test]
    fn migrate_chosen_text_to_option_id() {
        let mut p = sample_profile();
        // sample_profile sets chosen["CHOICE-RULE-1"] = "the alternative text".
        let unresolved = p.migrate_chosen_to_option_ids(&library_for_migration());
        assert!(unresolved.is_empty(), "exact directive text should resolve");
        assert_eq!(
            p.chosen.get("CHOICE-RULE-1").map(String::as_str),
            Some("alt"),
            "legacy alternative text migrates to the option id",
        );
        assert_eq!(p.version, PROFILE_VERSION);
    }

    #[test]
    fn migrate_leaves_already_migrated_option_id_untouched() {
        let mut p = sample_profile();
        p.chosen
            .insert("CHOICE-RULE-1".to_string(), "primary".to_string());
        let unresolved = p.migrate_chosen_to_option_ids(&library_for_migration());
        assert!(unresolved.is_empty());
        assert_eq!(
            p.chosen.get("CHOICE-RULE-1").map(String::as_str),
            Some("primary"),
        );
    }

    #[test]
    fn migrate_reports_unresolvable_text() {
        let mut p = sample_profile();
        p.chosen.insert(
            "CHOICE-RULE-1".to_string(),
            "text that matches no option".to_string(),
        );
        let unresolved = p.migrate_chosen_to_option_ids(&library_for_migration());
        assert_eq!(unresolved, vec!["CHOICE-RULE-1".to_string()]);
    }

    #[test]
    fn legacy_profile_without_selected_domains_still_loads() {
        // selected_domains was added later; profiles saved before its
        // introduction must still deserialize (the field carries
        // #[serde(default)]).
        let legacy_json = r#"{
            "version": 1,
            "selected_ids": ["UNIV-RULE-1"],
            "chosen": {},
            "custom_alternatives": {},
            "custom_rules": [],
            "custom_domains": [],
            "out_dir": "./out",
            "repos": [],
            "domain_repos": {}
        }"#;
        let p: Profile = serde_json::from_str(legacy_json).expect("legacy profile parses");
        assert_eq!(p.version, 1);
        assert!(
            p.selected_domains.is_empty(),
            "selected_domains defaults to empty for legacy profiles",
        );
        assert_eq!(p.selected_ids, vec!["UNIV-RULE-1".to_string()]);
    }
}
