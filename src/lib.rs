//! camerata core: the shared engine behind every frontend.
//!
//! The CLI (`src/main.rs`) and the GUI (`src/bin/gui.rs`) are both thin
//! frontends over these modules: load principles, select them, emit artifacts.

pub mod emit;
pub mod principle;
pub mod profile;
pub mod registry;

use std::path::PathBuf;

/// The principles directory bundled with the build (dev convenience default).
pub fn default_principles_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/principles"))
}

/// Capability domains: cross-cutting traits a project opts into (orthogonal to
/// language/stack). A rule on a capability applies to any project with it,
/// regardless of language. Grows as the library does.
pub const CAPABILITIES: &[&str] = &[
    "sql",
    "permissions",
    "iac",
    "api-layer",
    "agentic",
    "ci-cd",
    "concurrency",
];

/// Domains that ship pre-selected when the GUI launches. These are the
/// domains every camerata user opts into by default — Universal is
/// foundational philosophy, and Agentic is the operating-rule set for any
/// AI-driven development workflow.
///
/// This list is INTENTIONALLY curated and lives in code rather than in
/// per-rule TOML metadata. An SME contributing a new stack profile or
/// capability domain cannot promote their own domain into the default-
/// selected set; the PR linter enforces this. Adding a domain to this
/// list requires a separate, deliberate change to camerata-ai itself.
pub const DEFAULT_SELECTED_DOMAINS: &[&str] = &["*", "agentic"];

/// True if `domain` is in the default-selected set (Universal or Agentic).
pub fn is_default_selected_domain(domain: &str) -> bool {
    DEFAULT_SELECTED_DOMAINS.contains(&domain)
}

/// Whether a domain string names a capability (vs. a stack or "*").
pub fn is_capability(domain: &str) -> bool {
    CAPABILITIES.contains(&domain)
}

/// Human label for a domain, shared by the CLI and GUI.
pub fn domain_label(domain: &str) -> String {
    if domain == "*" {
        "Universal".to_string()
    } else if domain == "howto" {
        "Camerata · how to use".to_string()
    } else if domain == "contributing" {
        "Camerata · how to contribute a canonical rule".to_string()
    } else if is_capability(domain) {
        format!("Capability · {domain}")
    } else {
        format!("Stack · {domain}")
    }
}

/// True for the meta-documentation domains (`howto`, `contributing`) — the
/// ones whose principles are documentation about camerata itself rather
/// than installable conventions for a downstream project. Used by the GUI
/// to hide checkboxes, hide the + custom rule button, and skip the
/// "Choose how to adopt" controls in the detail pane.
pub fn is_meta_domain(domain: &str) -> bool {
    matches!(domain, "howto" | "contributing")
}
