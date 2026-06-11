//! The principle schema: the on-disk shape of a single principle definition.
//!
//! Source format is TOML (Rust-native, comment-friendly, good for hand-authoring
//! by contributors). serde decouples these in-memory types from the on-disk
//! format, so switching to JSON/YAML later is nearly free if ever needed.
//!
//! # The decision-first schema
//!
//! Every principle models a single architectural DECISION with a uniform list
//! of first-class OPTIONS. The `[decision]` block states the question and an
//! optional default; the `[[option]]` list carries every defensible position,
//! each with its own directive (the consumer-facing instruction) and `why`
//! (architect-facing rationale).
//!
//! The default is a flag *on the option list* (`decision.default` names an
//! option id), not a privileged sibling field. A decision with no default
//! (`decision.default` absent) is the route-to-human state: it cannot emit
//! until the architect resolves it, because the author is explicitly declining
//! to pretend there is a universal answer.

use serde::{Deserialize, Serialize};

/// Maps a principle onto the tool's selection behavior. Choice was retired:
/// every principle is now a decision with options, so the only distinction
/// that remains is whether the rule auto-adopts (universal) or is gated by a
/// stack/capability domain (stack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tag {
    /// 🌐 — auto-adopt, pre-checked, no prompt.
    Universal,
    /// 🔧 — stack-gated, only included if its domain's stack was selected.
    Stack,
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

/// Declares one artifact this principle emits and where it lands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Emit {
    /// e.g. "aicodingrules" (the interchange format) or a literal filename.
    pub target: String,
    /// Optional glob the rule applies to, e.g. "**/*.rs".
    #[serde(default)]
    pub scope: Option<String>,
}

/// The decision a principle models. Architect-facing; never emitted.
///
/// `default` names the option id adopted when the architect takes the rule
/// as-is. When it is `None`, the decision has no default: the rule is in the
/// route-to-human state and does not emit until the profile resolves it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Decision {
    /// What is being decided. Names the decision, not the winner.
    pub question: String,
    /// The option id adopted by default. Absent = no default (route-to-human).
    #[serde(default)]
    pub default: Option<String>,
    /// Why this decision matters. Architect-facing reasoning; never emitted.
    pub why: String,
}

/// One defensible position on a principle's decision. The `directive` is the
/// only consumer-facing field; `label` and `why` are architect-facing.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Opt {
    /// Stable per-option id, slug-cased, unique within the rule. Citable.
    pub id: String,
    /// Short human label for the option, shown in selection UI.
    pub label: String,
    /// The consumer-facing instruction emitted when this option is adopted.
    /// A single self-contained directive, plain prose, no opt-out paths.
    pub directive: String,
    /// Architect-facing rationale for this specific option (why it is or is
    /// not the default). Never emitted.
    pub why: String,
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
    /// (Distinct from `decision.default`, which names the adopted OPTION id;
    /// this `default` bool gates whether the rule is checked at all.) Whether a
    /// *domain* is auto-selected is controlled by `DEFAULT_SELECTED_DOMAINS`.
    pub default: bool,
    /// The decision this principle models. Architect-facing; never emitted.
    pub decision: Decision,
    /// Every defensible position on the decision, the default among them flagged
    /// by `decision.default`. At least one entry; the schema invites disagreement.
    #[serde(default, rename = "option")]
    pub options: Vec<Opt>,
    /// A deterministic conformance test that proves a project adheres to this
    /// rule's adopted directive: prose describing the check, or a runnable
    /// command (a grep pattern, a clippy/eslint lint, a CI invocation, a test).
    ///
    /// Architect-and-consumer-facing, but only for `mechanical` rules: it is
    /// emitted as a labelled "Conformance:" line attached to the rule's
    /// CONVENTIONS.md entry, where it operationalizes ORCH-CONFORMANCE-1 (a
    /// codified commitment is an enforced gate only if a deterministic check is
    /// wired into the pipeline). For `prose` and `structured` rules the field is
    /// accepted but never emitted: those enforcement levels have no deterministic
    /// gate to point the consumer agent at. Optional on every rule; the linter
    /// requires it on `mechanical` rules so the gate-text invariant holds.
    #[serde(default)]
    pub qualifies: Option<String>,
    #[serde(default)]
    pub emits: Vec<Emit>,
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

    /// True when this decision has no default option: the architect MUST resolve
    /// it at curation time, and the rule does not emit until they do.
    pub fn has_no_default(&self) -> bool {
        self.decision.default.is_none()
    }

    /// The option flagged as the default, if any.
    pub fn default_option(&self) -> Option<&Opt> {
        let id = self.decision.default.as_deref()?;
        self.options.iter().find(|o| o.id == id)
    }

    /// Look up an option by id.
    pub fn option(&self, id: &str) -> Option<&Opt> {
        self.options.iter().find(|o| o.id == id)
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

[decision]
question = "How is the thing done?"
default = "the-way"
why = "the architect-only reasoning for the decision"

[[option]]
id = "the-way"
label = "the canonical way"
directive = "A short directive."
why = "the canonical way is correct here"

[[option]]
id = "the-other-way"
label = "the loosened way"
directive = "the loosened version of this rule"
why = "looser; defensible only in narrow contexts"
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
        assert_eq!(p.options.len(), 2);
        assert_eq!(p.decision.question, "How is the thing done?");
    }

    #[test]
    fn domain_defaults_to_universal_star_when_absent() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert_eq!(p.domain, "*");
    }

    #[test]
    fn default_option_resolves_the_flagged_option() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert!(!p.has_no_default());
        let def = p.default_option().expect("default option present");
        assert_eq!(def.id, "the-way");
        assert_eq!(def.directive, "A short directive.");
    }

    #[test]
    fn no_default_decision_is_route_to_human() {
        let toml_text = r#"
id = "TEST-NODEF-1"
title = "A genuinely open decision"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "Which posture?"
why = "no universal answer; the project must decide"

[[option]]
id = "a"
label = "posture a"
directive = "do a"
why = "defensible when X"

[[option]]
id = "b"
label = "posture b"
directive = "do b"
why = "defensible when Y"
"#;
        let p: Principle = toml::from_str(toml_text).expect("parses");
        assert!(p.has_no_default());
        assert!(p.default_option().is_none());
        assert_eq!(p.options.len(), 2);
    }

    #[test]
    fn optional_fields_default_to_empty() {
        let p: Principle = toml::from_str(minimal_toml()).expect("parses");
        assert!(p.emits.is_empty());
        // qualifies is optional and absent on the minimal fixture.
        assert!(p.qualifies.is_none());
    }

    #[test]
    fn qualifies_parses_when_present() {
        let toml_text = r#"
id = "TEST-QUAL-1"
title = "a rule with a conformance test"
tag = "universal"
layer = "universal"
enforcement = "mechanical"
default = true
qualifies = "grep -r 'forbidden' src/ exits non-zero"

[decision]
question = "q"
default = "o"
why = "w"

[[option]]
id = "o"
label = "l"
directive = "d"
why = "w"
"#;
        let p: Principle = toml::from_str(toml_text).expect("parses");
        assert_eq!(
            p.qualifies.as_deref(),
            Some("grep -r 'forbidden' src/ exits non-zero")
        );
    }

    #[test]
    fn enum_tags_deserialize_lowercase() {
        for (literal, expected) in [("universal", Tag::Universal), ("stack", Tag::Stack)] {
            let toml_text = format!(
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"{literal}\"\nlayer = \"universal\"\nenforcement = \"prose\"\ndefault = false\n[decision]\nquestion = \"q\"\ndefault = \"o\"\nwhy = \"w\"\n[[option]]\nid = \"o\"\nlabel = \"l\"\ndirective = \"d\"\nwhy = \"w\"\n"
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
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"universal\"\nlayer = \"universal\"\nenforcement = \"{literal}\"\ndefault = false\n[decision]\nquestion = \"q\"\ndefault = \"o\"\nwhy = \"w\"\n[[option]]\nid = \"o\"\nlabel = \"l\"\ndirective = \"d\"\nwhy = \"w\"\n"
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
                "id = \"X-Y-1\"\ntitle = \"t\"\ntag = \"universal\"\nlayer = \"{literal}\"\nenforcement = \"prose\"\ndefault = false\n[decision]\nquestion = \"q\"\ndefault = \"o\"\nwhy = \"w\"\n[[option]]\nid = \"o\"\nlabel = \"l\"\ndirective = \"d\"\nwhy = \"w\"\n"
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
