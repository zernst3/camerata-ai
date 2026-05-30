//! The principle schema: the on-disk shape of a single principle definition.
//!
//! Source format is TOML (Rust-native, comment-friendly, good for hand-authoring
//! by contributors). serde decouples these in-memory types from the on-disk
//! format, so switching to JSON/YAML later is nearly free if ever needed.

use serde::{Deserialize, Serialize};

/// Maps the doc's tags (🌐 / 🔧 / ⚖️) onto the tool's selection behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tag {
    /// 🌐 — auto-adopt, pre-checked, no prompt.
    Universal,
    /// 🔧 — stack-gated, only included if its domain's stack was selected.
    Stack,
    /// ⚖️ — prompt the user; present the default + alternatives and let them pick.
    Choice,
}

/// Precedence layer. Declaration order IS the precedence order (derived `Ord`):
/// Universal < Language < Library < Framework. Most-specific wins on conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Universal,
    Language,
    Library,
    Framework,
}

/// How strongly a principle is enforced once emitted. Weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Enforcement {
    /// Context guidance the agent should follow (e.g. a CLAUDE.md rule).
    Prose,
    /// A referenceable rule with an ID (e.g. a CONVENTIONS.md entry).
    Structured,
    /// A deterministic gate the agent cannot bypass (lint/hook/CI/type system).
    Mechanical,
}

/// A ⚖️ guided choice: the part that, by design, the human decides.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Choice {
    pub prompt: String,
    pub options: Vec<String>,
    pub default: String,
}

/// Declares one artifact this principle emits and where it lands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Emit {
    /// e.g. "aicodingrules" (the interchange format) or a literal filename.
    pub target: String,
    /// Optional glob the rule applies to, e.g. "**/*.rs".
    #[serde(default)]
    pub scope: Option<String>,
}

/// One principle definition, deserialized from a `principles/*.toml` file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Principle {
    pub id: String,
    pub title: String,
    pub tag: Tag,
    /// "*" = universal; else a stack path like "rust", "rust:seaorm", "rust:dioxus".
    #[serde(default = "default_domain")]
    pub domain: String,
    pub layer: Layer,
    pub enforcement: Enforcement,
    /// Whether this rule auto-checks when its domain is selected. SMEs set
    /// this per rule; the field is required so the call is always deliberate.
    /// Whether a *domain* is auto-selected is controlled by
    /// `DEFAULT_SELECTED_DOMAINS` in lib.rs (curated, not author-settable).
    pub default: bool,
    #[serde(default)]
    pub stance: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub emits: Vec<Emit>,
    #[serde(default)]
    pub choice: Option<Choice>,
}

impl Principle {
    /// The stack this principle belongs to, e.g. "rust" for domain "rust:seaorm".
    /// Returns None for universal principles.
    pub fn stack_base(&self) -> Option<&str> {
        if self.domain == "*" {
            None
        } else {
            Some(self.domain.split(':').next().unwrap_or(&self.domain))
        }
    }
}

fn default_domain() -> String {
    "*".to_string()
}
