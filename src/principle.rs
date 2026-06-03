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

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_toml() -> &'static str {
        r#"
id = "TEST-MINIMAL-1"
title = "Minimal rule for round-trip"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true
summary = "A short directive."
alternatives = ["the loosened version of this rule"]
"#
    }

    #[test]
    fn toml_parses_minimal_principle() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert_eq!(p.id, "TEST-MINIMAL-1");
        assert_eq!(p.tag, Tag::Universal);
        assert_eq!(p.layer, Layer::Universal);
        assert_eq!(p.enforcement, Enforcement::Prose);
        assert!(p.default);
        assert_eq!(p.alternatives.len(), 1);
    }

    #[test]
    fn domain_defaults_to_universal_star_when_absent() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert_eq!(p.domain, "*");
    }

    #[test]
    fn optional_fields_default_to_empty() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert!(p.stance.is_none());
        assert!(p.why.is_none());
        assert!(p.emits.is_empty());
        assert!(p.choice.is_none());
    }

    #[test]
    fn enum_tags_deserialize_lowercase() {
        for (literal, expected) in [
            ("universal", Tag::Universal),
            ("stack", Tag::Stack),
            ("choice", Tag::Choice),
        ] {
            let toml_text = format!(
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"{literal}\"\nlayer = \"universal\"\nenforcement = \"prose\"\ndefault = false\nsummary = \"s\"\nalternatives = [\"a\"]\n"
            );
            let p: Principle = toml::from_str(&toml_text).expect("parses");
            assert_eq!(p.tag, expected, "tag literal {literal}");
        }
    }

    #[test]
    fn enum_enforcement_deserialize_lowercase() {
        for (literal, expected) in [
            ("prose", Enforcement::Prose),
            ("structured", Enforcement::Structured),
            ("mechanical", Enforcement::Mechanical),
        ] {
            let toml_text = format!(
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"universal\"\nlayer = \"universal\"\nenforcement = \"{literal}\"\ndefault = false\nsummary = \"s\"\nalternatives = [\"a\"]\n"
            );
            let p: Principle = toml::from_str(&toml_text).expect("parses");
            assert_eq!(p.enforcement, expected, "enforcement literal {literal}");
        }
    }

    #[test]
    fn enum_layer_deserialize_lowercase() {
        for (literal, expected) in [
            ("universal", Layer::Universal),
            ("language", Layer::Language),
            ("library", Layer::Library),
            ("framework", Layer::Framework),
        ] {
            let toml_text = format!(
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"universal\"\nlayer = \"{literal}\"\nenforcement = \"prose\"\ndefault = false\nsummary = \"s\"\nalternatives = [\"a\"]\n"
            );
            let p: Principle = toml::from_str(&toml_text).expect("parses");
            assert_eq!(p.layer, expected, "layer literal {literal}");
        }
    }

    #[test]
    fn layer_orders_universal_before_specialized() {
        // Most-specific wins on conflict; the derived Ord must match the
        // declaration order so emit.rs's layer sort puts universal first.
        assert!(Layer::Universal < Layer::Language);
        assert!(Layer::Language < Layer::Library);
        assert!(Layer::Library < Layer::Framework);
    }

    #[test]
    fn choice_block_parses_when_present() {
        let toml_text = r#"
id = "X-Y-1"
title = "t"
tag = "choice"
layer = "universal"
enforcement = "prose"
default = false
summary = "s"
alternatives = ["a"]

[choice]
prompt = "Pick one"
options = ["one", "two"]
default = "one"
"#;
        let p: Principle = toml::from_str(toml_text).expect("parses");
        let c = p.choice.expect("choice present");
        assert_eq!(c.prompt, "Pick one");
        assert_eq!(c.options, vec!["one", "two"]);
        assert_eq!(c.default, "one");
    }

    #[test]
    fn stack_base_returns_none_for_universal() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert_eq!(p.stack_base(), None);
    }

    #[test]
    fn stack_base_strips_colon_suffix() {
        let mut p: Principle = toml::from_str(minimal_toml()).expect("parses");
        p.domain = "rust".to_string();
        assert_eq!(p.stack_base(), Some("rust"));
        p.domain = "rust:seaorm".to_string();
        assert_eq!(p.stack_base(), Some("rust"));
        p.domain = "rust:dioxus".to_string();
        assert_eq!(p.stack_base(), Some("rust"));
    }
}
