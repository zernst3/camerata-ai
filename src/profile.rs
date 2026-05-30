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
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const PROFILE_VERSION: u32 = 1;

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
        let json = serde_json::to_string_pretty(self)
            .context("serializing profile to JSON")?;
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
}
