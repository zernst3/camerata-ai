//! Turns selected principles into files on disk, plus a lockfile recording
//! exactly what was installed (the foundation for future `outdated` checks).

use crate::principle::{Enforcement, Principle};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// A principle the user chose. `chosen` is None to take the rule's default
/// option, or Some(option_id) to take a specific option by id. The id may name
/// a library option or a custom option the architect authored (in which case
/// the directive text is supplied via `custom_directive`).
///
/// For rules with no default (`decision.default` absent), `chosen` MUST resolve
/// to a real option: an unresolved no-default rule cannot emit, because there
/// is nothing to fall back to (see `resolve_directive`).
pub struct Selection<'a> {
    pub principle: &'a Principle,
    pub chosen: Option<String>,
    /// Directive text for a custom option whose id is not in the library (the
    /// architect authored their own option). When `chosen` names an id absent
    /// from the library, this text is the body. Library options resolve by id.
    pub custom_directive: Option<String>,
}

impl<'a> Selection<'a> {
    /// Construct a selection that takes the rule's default option.
    pub fn new(principle: &'a Principle) -> Self {
        Selection {
            principle,
            chosen: None,
            custom_directive: None,
        }
    }

    /// The directive text to emit for this selection, or None when the rule has
    /// no default and the architect has not resolved it (route-to-human state).
    ///
    /// Resolution order: an explicit custom directive wins; else the chosen
    /// option id is looked up in the library; else the rule's default option;
    /// else None (no default, unresolved).
    pub fn resolve_directive(&self) -> Option<&str> {
        if let Some(text) = &self.custom_directive {
            return Some(text.as_str());
        }
        match &self.chosen {
            Some(id) => self.principle.option(id).map(|o| o.directive.as_str()),
            None => self
                .principle
                .default_option()
                .map(|o| o.directive.as_str()),
        }
    }
}

/// A free-text rule the user wrote that isn't in the library. Attaches to a
/// domain ("*" universal, or a stack like "rust") so it can be added anywhere.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CustomRule {
    pub name: String,
    pub body: String,
    pub domain: String,
}

/// Serializable view of a resolved selection: the full principle plus the
/// chosen option. This is the JSON export of the user's decisions.
#[derive(Serialize)]
pub struct SelectionRecord<'a> {
    #[serde(flatten)]
    pub principle: &'a Principle,
    pub chosen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_directive: Option<String>,
}

/// Render the user's resolved decisions as pretty JSON (machine-interchange).
pub fn selections_json(selections: &[Selection]) -> Result<String> {
    let records: Vec<SelectionRecord> = selections
        .iter()
        .map(|s| SelectionRecord {
            principle: s.principle,
            chosen: s.chosen.clone(),
            custom_directive: s.custom_directive.clone(),
        })
        .collect();
    Ok(serde_json::to_string_pretty(&records)?)
}

/// Render the whole library as pretty JSON (catalog export).
pub fn catalog_json(principles: &[Principle]) -> Result<String> {
    Ok(serde_json::to_string_pretty(principles)?)
}

/// What `scaffold` wrote, for the CLI summary.
pub struct Outcome {
    pub files: Vec<(String, usize)>, // (filename, principle count)
    pub installed: usize,
}

/// Map an emit target to the filename it lands in.
fn target_filename(target: &str) -> &str {
    match target {
        // AGENTS.md is the cross-tool open standard (Linux Foundation /
        // Agentic AI Foundation), read by Claude Code, Cursor, Codex,
        // Copilot, Sourcegraph, and others. `aicodingrules` here is the
        // historical interchange identifier in the principle TOMLs; the
        // file it lands in is AGENTS.md.
        "aicodingrules" => "AGENTS.md",
        other => other,
    }
}

/// Where a principle lands when it declares no explicit emits: by enforcement.
fn default_target(p: &Principle) -> &'static str {
    match p.enforcement {
        Enforcement::Prose => "aicodingrules",
        Enforcement::Structured | Enforcement::Mechanical => "CONVENTIONS.md",
    }
}

/// Render one principle as a markdown rule block (optionally scoped to a glob).
/// Returns None when the rule has no default and the architect has not resolved
/// it: a route-to-human decision does not emit until a position is chosen.
///
/// The emitted block represents the project's *adopted* position on this
/// principle, stated as a single directive (the resolved option's `directive`)
/// the consumer agent follows at code-author time.
///
/// Architect-only fields are deliberately NOT emitted: the `[decision]` block,
/// every option's `label` and `why`, and the non-adopted options. Each of those
/// exists to support the architect at curation time (picking which option to
/// adopt, reviewing rules in PRs, understanding the reasoning), and including
/// them in the consumer agent's input would introduce interpretation surfaces
/// that compete with the directive itself. The consumer agent must see one
/// unambiguous instruction; the emit is tuned for that determinism, not for
/// transparency to a human reading the generated file.
pub fn render(sel: &Selection, scope: Option<&str>) -> Option<String> {
    let p = sel.principle;
    let body = sel.resolve_directive()?;
    let mut s = String::new();
    s.push_str(&format!("### {} — {}\n", p.id, p.title));
    s.push_str(&format!("{body}\n"));
    if let Some(scope) = scope {
        s.push_str(&format!("\n_Applies to:_ `{scope}`\n"));
    }
    s.push('\n');
    Some(s)
}

/// A stable hash of an arbitrary string, for the lockfile.
fn content_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// A stable content hash of a principle's meaning AS ADOPTED, for the lockfile.
/// Hashes the id, title, the resolved option id, and the resolved directive, so
/// `outdated` fires when the upstream rule's adopted directive text changes.
/// (std hasher; no crypto dep for v0.1.)
fn principle_hash(sel: &Selection) -> String {
    let p = sel.principle;
    let mut h = DefaultHasher::new();
    p.id.hash(&mut h);
    p.title.hash(&mut h);
    // The resolved option id (default, chosen, or "custom"), then its directive.
    let resolved_id = sel
        .chosen
        .clone()
        .or_else(|| p.decision.default.clone())
        .unwrap_or_else(|| "custom".to_string());
    resolved_id.hash(&mut h);
    if let Some(directive) = sel.resolve_directive() {
        directive.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// The lockfile hash for a principle adopted at its DEFAULT option. `outdated`
/// compares against this because the lockfile records only ids and hashes, not
/// which option each project chose; the default-adopted hash is the right
/// comparison for the common case (defaulted rules). A rule the architect
/// resolved to a non-default option will read as "changed" here, which is a
/// conservative (false-positive-leaning) signal, never a missed change.
pub fn default_principle_hash(p: &Principle) -> String {
    principle_hash(&Selection::new(p))
}

/// One difference between an installed lockfile and the current library.
#[derive(Debug, PartialEq, Eq)]
pub enum Drift {
    /// The rule's adopted hash changed upstream (id present in both).
    Changed(String),
    /// The rule is installed but no longer exists in the current library.
    Removed(String),
}

/// Parse the `[[installed]]` id/hash pairs out of a `camerata.lock` body.
pub fn parse_lock(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur_id: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("id = ") {
            cur_id = Some(rest.trim().trim_matches('"').to_string());
        } else if let Some(rest) = line.strip_prefix("hash = ") {
            if let Some(id) = cur_id.take() {
                out.push((id, rest.trim().trim_matches('"').to_string()));
            }
        }
    }
    out
}

/// Compare a parsed lockfile against the current library and report drift.
/// Returns one entry per installed rule whose adopted hash changed or that no
/// longer exists. Rules whose hash is unchanged, and library rules not in the
/// lock, are omitted. Custom rules (id starting with `CUSTOM-`) are skipped
/// because their content is user-owned, not upstream.
pub fn outdated(installed: &[(String, String)], library: &[Principle]) -> Vec<Drift> {
    let by_id: HashMap<&str, &Principle> = library.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut drift = Vec::new();
    for (id, locked_hash) in installed {
        if id.starts_with("CUSTOM-") {
            continue;
        }
        match by_id.get(id.as_str()) {
            None => drift.push(Drift::Removed(id.clone())),
            Some(p) => {
                if default_principle_hash(p) != *locked_hash {
                    drift.push(Drift::Changed(id.clone()));
                }
            }
        }
    }
    drift
}

/// Build the self-describing header that prefixes each emitted file.
///
/// The header has two roles. First, it tells a consumer AI agent that
/// has loaded the file (without other context) exactly what the file
/// is for, what to do with the directives below, and where its
/// companion file fits. Second, it tells a human reader that the file
/// is generated and must not be edited by hand.
///
/// The header content is keyed off the filename so AGENTS.md and
/// CONVENTIONS.md each describe their own role accurately. Unknown
/// filenames fall back to the generic generated-by header.
fn file_header(filename: &str, count: usize) -> String {
    let generated_line = format!(
        "<!-- Generated by camerata. {count} principle(s). Edit principle sources, not this file. -->\n\n"
    );
    let context = match filename {
        "AGENTS.md" => "\
# AGENTS.md

This file is the authoritative source of architectural and behavioral commitments the AI agent applies when writing or modifying code in this project. Each block below states a single directive the agent follows at code-author time.

These are prose-enforcement rules. The agent reads and respects them by judgment, not by lint or compiler check. They were adopted from the camerata principle library by the project's architect. The rules represent committed choices, not suggestions.

The companion file CONVENTIONS.md holds this project's structured and mechanical conventions, organized by citable rule ID. The agent reads both files when starting work and cites the rule IDs from CONVENTIONS.md when referencing them in commits or PR descriptions.

Do not edit this file by hand. To change the rules, edit the project's principle selection and regenerate via camerata.

---

",
        "CONVENTIONS.md" => "\
# CONVENTIONS.md

This file is the authoritative source of structured and mechanical conventions this project follows. Each block below states a rule with a stable citable ID that reviewers and the AI agent reference in commits, PR descriptions, and code reviews.

These conventions were adopted from the camerata principle library by the project's architect. The rules represent committed choices, not suggestions. Structured rules are verified by code review and citation; mechanical rules can be enforced by lint, type-system, or CI checks.

The companion file AGENTS.md holds this project's prose-enforcement directives, the behavioral guidance the agent applies by judgment. The agent reads both files when starting work.

When applying a rule, cite its ID in the commit body or PR description (for example, per RUST-DOMAIN-6).

Do not edit this file by hand. To change the rules, edit the project's principle selection and regenerate via camerata.

---

",
        _ => "",
    };
    format!("{generated_line}{context}")
}

/// Write the selected principles + any custom rules + a lockfile into `out_dir`.
pub fn scaffold(
    out_dir: &Path,
    selections: &[Selection],
    custom: &[CustomRule],
) -> Result<Outcome> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir `{}`", out_dir.display()))?;

    // filename -> accumulated buffer
    let mut buffers: BTreeMap<String, String> = BTreeMap::new();
    // filename -> count of principles contributing
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    // lockfile entries: (id, hash)
    let mut installed: Vec<(String, String)> = Vec::new();

    // Emit in precedence order (universal -> language -> library -> framework)
    // so more-general rules appear before the more-specific ones that refine them.
    let mut ordered: Vec<&Selection> = selections.iter().collect();
    ordered.sort_by(|a, b| {
        a.principle
            .layer
            .cmp(&b.principle.layer)
            .then_with(|| a.principle.id.cmp(&b.principle.id))
    });

    for sel in ordered {
        // Skip rules with no default that the architect has not resolved: a
        // route-to-human decision does not emit until a position is chosen.
        if render(sel, None).is_none() {
            continue;
        }

        // Resolve target files (+ optional scope): explicit emits, else the
        // enforcement default.
        let targets: Vec<(String, Option<String>)> = if sel.principle.emits.is_empty() {
            vec![(default_target(sel.principle).to_string(), None)]
        } else {
            sel.principle
                .emits
                .iter()
                .map(|e| (e.target.clone(), e.scope.clone()))
                .collect()
        };

        for (target, scope) in targets {
            // Safe: the no-default skip above guarantees render returns Some.
            let Some(block) = render(sel, scope.as_deref()) else {
                continue;
            };
            let filename = target_filename(&target).to_string();
            buffers
                .entry(filename.clone())
                .or_default()
                .push_str(&block);
            *counts.entry(filename).or_insert(0) += 1;
        }

        installed.push((sel.principle.id.clone(), principle_hash(sel)));
    }

    // Append any custom (user-authored) rules, grouped under their domain.
    for c in custom {
        if c.name.trim().is_empty() && c.body.trim().is_empty() {
            continue;
        }
        let domain = if c.domain.is_empty() {
            "*"
        } else {
            c.domain.as_str()
        };
        let block = format!(
            "### CUSTOM-{} _(custom · domain: {})_\n{}\n\n",
            c.name.trim(),
            domain,
            c.body.trim()
        );
        // Custom rules emit to AGENTS.md alongside the canonical prose
        // rules. They are user-authored free-text guidance with no
        // enforcement level, so they live in the same file the AI agent
        // reads as its primary instruction surface. CLAUDE.md was the
        // legacy default from before AGENTS.md became the cross-tool
        // standard.
        let filename = target_filename("aicodingrules").to_string();
        buffers
            .entry(filename.clone())
            .or_default()
            .push_str(&block);
        *counts.entry(filename).or_insert(0) += 1;
        installed.push((format!("CUSTOM-{}", c.name.trim()), content_hash(&block)));
    }

    // Write each rules file with a generated header.
    let mut files = Vec::new();
    for (filename, body) in &buffers {
        let path = out_dir.join(filename);
        let count = counts.get(filename).copied().unwrap_or(0);
        let header = file_header(filename, count);
        fs::write(&path, format!("{header}{body}"))
            .with_context(|| format!("writing `{}`", path.display()))?;
        files.push((filename.clone(), count));
    }

    // Write the lockfile.
    let mut lock = String::from(
        "# camerata.lock — generated. Records installed principles + content hashes.\n\
         # Do not edit by hand; used to detect upstream updates (`camerata outdated`).\n\n",
    );
    for (id, hash) in &installed {
        lock.push_str(&format!(
            "[[installed]]\nid = \"{id}\"\nhash = \"{hash}\"\n\n"
        ));
    }
    let lock_path = out_dir.join("camerata.lock");
    fs::write(&lock_path, lock).with_context(|| format!("writing `{}`", lock_path.display()))?;

    Ok(Outcome {
        files,
        installed: installed.len(),
    })
}

/// Route selections + custom rules to per-domain output directories, falling
/// back to `default_out` for any domain without an override. Groups by resolved
/// target and scaffolds each target once. Returns one Outcome per target.
///
/// This is how a Rust repo and an Infra repo each receive only their own rules.
pub fn scaffold_routed(
    default_out: &Path,
    overrides: &HashMap<String, Vec<PathBuf>>,
    selections: &[Selection],
    custom: &[CustomRule],
) -> Result<Vec<(PathBuf, Outcome)>> {
    // A domain maps to one OR MANY repos; an unmapped domain uses the default.
    let targets_for = |domain: &str| -> Vec<PathBuf> {
        match overrides.get(domain) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => vec![default_out.to_path_buf()],
        }
    };

    // Bucket by resolved target directory (a selection can land in several).
    let mut buckets: BTreeMap<PathBuf, (Vec<Selection>, Vec<CustomRule>)> = BTreeMap::new();
    for s in selections {
        for target in targets_for(&s.principle.domain) {
            buckets.entry(target).or_default().0.push(Selection {
                principle: s.principle,
                chosen: s.chosen.clone(),
                custom_directive: s.custom_directive.clone(),
            });
        }
    }
    for c in custom {
        let domain = if c.domain.is_empty() {
            "*"
        } else {
            c.domain.as_str()
        };
        for target in targets_for(domain) {
            buckets.entry(target).or_default().1.push(c.clone());
        }
    }

    let mut results = Vec::new();
    for (target, (sels, customs)) in buckets {
        let outcome = scaffold(&target, &sels, &customs)?;
        results.push((target, outcome));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::principle::{Layer, Principle};
    use tempfile::tempdir;

    /// Build a new-schema principle with a default option whose directive is
    /// `directive`, plus one non-default option. The default option's id is
    /// "primary"; the alternative's id is "alt".
    fn principle(
        id: &str,
        title: &str,
        domain: &str,
        layer: Layer,
        enforcement: Enforcement,
        directive: &str,
    ) -> Principle {
        let toml_text = format!(
            r#"
id = "{id}"
title = "{title}"
tag = "universal"
domain = "{domain}"
layer = "{layer}"
enforcement = "{enforcement}"
default = true

[decision]
question = "the decision question this rule models"
default = "primary"
why = "the architect-only reasoning that must not appear in emitted output"

[[option]]
id = "primary"
label = "the canonical option"
directive = "{directive}"
why = "the architect-only per-option reasoning that must not appear in emit"

[[option]]
id = "alt"
label = "the loosened option"
directive = "the loosened variant that is the alternative directive"
why = "looser; defensible only in narrow contexts"
"#,
            layer = match layer {
                Layer::Universal => "universal",
                Layer::Language => "language",
                Layer::Library => "library",
                Layer::Framework => "framework",
            },
            enforcement = match enforcement {
                Enforcement::Prose => "prose",
                Enforcement::Structured => "structured",
                Enforcement::Mechanical => "mechanical",
            },
        );
        toml::from_str(&toml_text).expect("fixture parses")
    }

    #[test]
    fn target_filename_maps_aicodingrules_to_agents_md() {
        assert_eq!(target_filename("aicodingrules"), "AGENTS.md");
    }

    #[test]
    fn target_filename_passes_other_targets_through() {
        assert_eq!(target_filename("CONVENTIONS.md"), "CONVENTIONS.md");
        assert_eq!(target_filename("CUSTOM_FILE.md"), "CUSTOM_FILE.md");
    }

    #[test]
    fn default_target_routes_prose_to_agents_via_interchange_id() {
        let p = principle(
            "TEST-PROSE-1",
            "prose rule",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "a prose directive",
        );
        assert_eq!(default_target(&p), "aicodingrules");
    }

    #[test]
    fn default_target_routes_structured_and_mechanical_to_conventions_md() {
        for enforcement in [Enforcement::Structured, Enforcement::Mechanical] {
            let p = principle(
                "TEST-X-1",
                "rule",
                "*",
                Layer::Universal,
                enforcement,
                "a rule",
            );
            assert_eq!(default_target(&p), "CONVENTIONS.md");
        }
    }

    #[test]
    fn render_emits_id_and_title_header() {
        let p = principle(
            "TEST-RENDER-1",
            "the rule title",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the directive",
        );
        let out = render(&Selection::new(&p), None).expect("default option renders");
        assert!(
            out.starts_with("### TEST-RENDER-1 — the rule title\n"),
            "header missing or malformed; got:\n{out}",
        );
    }

    #[test]
    fn render_uses_default_option_directive_when_nothing_chosen() {
        let p = principle(
            "TEST-DEFAULT-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the default option directive",
        );
        let out = render(&Selection::new(&p), None).expect("renders");
        assert!(out.contains("the default option directive"));
    }

    #[test]
    fn render_substitutes_chosen_option_directive_for_default() {
        let p = principle(
            "TEST-CHOICE-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the default option directive",
        );
        let out = render(
            &Selection {
                principle: &p,
                chosen: Some("alt".to_string()),
                custom_directive: None,
            },
            None,
        )
        .expect("renders");
        assert!(out.contains("the loosened variant that is the alternative directive"));
        assert!(
            !out.contains("the default option directive"),
            "default directive leaked when an alternative was chosen; got:\n{out}",
        );
    }

    #[test]
    fn render_uses_custom_directive_verbatim() {
        let p = principle(
            "TEST-CUSTOM-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the default option directive",
        );
        let out = render(
            &Selection {
                principle: &p,
                chosen: Some("my-own".to_string()),
                custom_directive: Some("the architect's own directive".to_string()),
            },
            None,
        )
        .expect("renders");
        assert!(out.contains("the architect's own directive"));
    }

    #[test]
    fn render_returns_none_for_unresolved_no_default_rule() {
        // A rule with no decision.default and no chosen option is route-to-human
        // and must not emit.
        let toml_text = r#"
id = "TEST-NODEF-1"
title = "open decision"
tag = "universal"
domain = "*"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "which posture?"
why = "no universal answer"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "defensible when X"

[[option]]
id = "b"
label = "b"
directive = "do b"
why = "defensible when Y"
"#;
        let p: Principle = toml::from_str(toml_text).expect("parses");
        assert!(render(&Selection::new(&p), None).is_none());
        // But once resolved, it renders.
        let resolved = render(
            &Selection {
                principle: &p,
                chosen: Some("a".to_string()),
                custom_directive: None,
            },
            None,
        )
        .expect("renders once resolved");
        assert!(resolved.contains("do a"));
    }

    #[test]
    fn render_omits_architect_only_fields() {
        // The decision block, option labels, option whys, and non-adopted
        // options are architect-only and must never reach the consumer agent.
        let p = principle(
            "TEST-OMIT-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the directive",
        );
        let out = render(&Selection::new(&p), None).expect("renders");
        assert!(
            !out.contains("architect-only reasoning"),
            "decision why leaked"
        );
        assert!(!out.contains("per-option reasoning"), "option why leaked");
        assert!(
            !out.contains("loosened variant"),
            "non-adopted option leaked"
        );
        assert!(!out.contains("decision question"), "question leaked");
    }

    #[test]
    fn render_byte_identical_to_legacy_default_emit_shape() {
        // The new schema must emit byte-identically to the old shape for a rule
        // adopted at its default: "### ID — TITLE\n<directive>\n\n".
        let p = principle(
            "ARCH-CURSOR-PAGINATION-1",
            "List endpoints paginate by cursor, not by offset",
            "api-layer",
            Layer::Universal,
            Enforcement::Structured,
            "Any list endpoint that can grow uses opaque cursor tokens.",
        );
        let out = render(&Selection::new(&p), None).expect("renders");
        let expected = "### ARCH-CURSOR-PAGINATION-1 — List endpoints paginate by cursor, not by offset\nAny list endpoint that can grow uses opaque cursor tokens.\n\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn render_appends_scope_line_when_provided() {
        let p = principle(
            "TEST-SCOPE-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "directive",
        );
        let out = render(&Selection::new(&p), Some("**/*.rs")).expect("renders");
        assert!(out.contains("_Applies to:_ `**/*.rs`"));
    }

    #[test]
    fn scaffold_partitions_prose_to_agents_md_and_structured_to_conventions_md() {
        let dir = tempdir().expect("tempdir");
        let prose = principle(
            "TEST-PROSE-1",
            "prose rule",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "prose directive body",
        );
        let structured = principle(
            "TEST-STRUCT-1",
            "structured rule",
            "*",
            Layer::Universal,
            Enforcement::Structured,
            "structured directive body",
        );
        let selections = vec![Selection::new(&prose), Selection::new(&structured)];

        let outcome = scaffold(dir.path(), &selections, &[]).expect("scaffold");
        assert_eq!(outcome.installed, 2);

        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).expect("AGENTS.md exists");
        let conv =
            fs::read_to_string(dir.path().join("CONVENTIONS.md")).expect("CONVENTIONS.md exists");
        assert!(
            agents.contains("TEST-PROSE-1"),
            "prose rule landed in AGENTS.md"
        );
        assert!(
            !agents.contains("TEST-STRUCT-1"),
            "structured rule must NOT land in AGENTS.md",
        );
        assert!(
            conv.contains("TEST-STRUCT-1"),
            "structured rule landed in CONVENTIONS.md",
        );
        assert!(
            !conv.contains("TEST-PROSE-1"),
            "prose rule must NOT land in CONVENTIONS.md",
        );
    }

    #[test]
    fn scaffold_skips_unresolved_no_default_rule() {
        let dir = tempdir().expect("tempdir");
        let toml_text = r#"
id = "TEST-NODEF-2"
title = "open decision"
tag = "universal"
domain = "*"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "which posture?"
why = "no universal answer"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "defensible when X"

[[option]]
id = "b"
label = "b"
directive = "do b"
why = "defensible when Y"
"#;
        let nodef: Principle = toml::from_str(toml_text).expect("parses");
        let resolved = principle(
            "TEST-DEF-2",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "resolved directive body",
        );
        let outcome = scaffold(
            dir.path(),
            &[Selection::new(&nodef), Selection::new(&resolved)],
            &[],
        )
        .expect("scaffold");
        // Only the resolved rule installs; the no-default rule is skipped.
        assert_eq!(outcome.installed, 1);
        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).expect("AGENTS.md");
        assert!(agents.contains("TEST-DEF-2"));
        assert!(!agents.contains("TEST-NODEF-2"));
    }

    #[test]
    fn scaffold_orders_selections_by_layer_ascending_then_id() {
        let dir = tempdir().expect("tempdir");
        let universal = principle(
            "AAA-UNIV-1",
            "universal",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "universal body",
        );
        let language = principle(
            "AAA-LANG-1",
            "language",
            "rust",
            Layer::Language,
            Enforcement::Prose,
            "language body",
        );
        let framework = principle(
            "AAA-FRAME-1",
            "framework",
            "rust:dioxus",
            Layer::Framework,
            Enforcement::Prose,
            "framework body",
        );
        let selections = vec![
            Selection::new(&framework),
            Selection::new(&universal),
            Selection::new(&language),
        ];

        scaffold(dir.path(), &selections, &[]).expect("scaffold");
        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).expect("AGENTS.md");
        let univ_pos = agents.find("AAA-UNIV-1").expect("universal present");
        let lang_pos = agents.find("AAA-LANG-1").expect("language present");
        let frame_pos = agents.find("AAA-FRAME-1").expect("framework present");
        assert!(univ_pos < lang_pos, "universal must precede language");
        assert!(lang_pos < frame_pos, "language must precede framework");
    }

    #[test]
    fn scaffold_writes_lockfile_with_each_installed_id() {
        let dir = tempdir().expect("tempdir");
        let p = principle(
            "TEST-LOCK-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "body",
        );
        let selections = vec![Selection::new(&p)];
        scaffold(dir.path(), &selections, &[]).expect("scaffold");

        let lock = fs::read_to_string(dir.path().join("camerata.lock")).expect("lockfile exists");
        assert!(lock.contains("[[installed]]"));
        assert!(lock.contains("id = \"TEST-LOCK-1\""));
        assert!(lock.contains("hash = "));
    }

    #[test]
    fn scaffold_emits_custom_rule_into_agents_md_under_custom_heading() {
        let dir = tempdir().expect("tempdir");
        let custom = CustomRule {
            name: "my-rule".to_string(),
            body: "do the thing".to_string(),
            domain: "rust".to_string(),
        };
        scaffold(dir.path(), &[], &[custom]).expect("scaffold");

        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).expect("AGENTS.md");
        assert!(agents.contains("### CUSTOM-my-rule"));
        assert!(agents.contains("domain: rust"));
        assert!(agents.contains("do the thing"));
    }

    #[test]
    fn scaffold_skips_empty_custom_rules() {
        let dir = tempdir().expect("tempdir");
        let blank = CustomRule {
            name: "   ".to_string(),
            body: "\n".to_string(),
            domain: "*".to_string(),
        };
        scaffold(dir.path(), &[], &[blank]).expect("scaffold");
        // No selections + skipped custom = no AGENTS.md written at all.
        assert!(!dir.path().join("AGENTS.md").exists());
    }

    #[test]
    fn scaffold_routed_sends_each_domain_to_its_override() {
        let root = tempdir().expect("tempdir");
        let default_out = root.path().join("default");
        let rust_out = root.path().join("rust-repo");

        let univ = principle(
            "ROUTE-UNIV-1",
            "universal",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "universal body",
        );
        let rust = principle(
            "ROUTE-RUST-1",
            "rust",
            "rust",
            Layer::Language,
            Enforcement::Prose,
            "rust body",
        );
        let selections = vec![Selection::new(&univ), Selection::new(&rust)];
        let mut overrides: HashMap<String, Vec<PathBuf>> = HashMap::new();
        overrides.insert("rust".to_string(), vec![rust_out.clone()]);

        scaffold_routed(&default_out, &overrides, &selections, &[]).expect("scaffold_routed");

        let default_agents =
            fs::read_to_string(default_out.join("AGENTS.md")).expect("default AGENTS.md");
        let rust_agents = fs::read_to_string(rust_out.join("AGENTS.md")).expect("rust AGENTS.md");
        assert!(default_agents.contains("ROUTE-UNIV-1"));
        assert!(
            !default_agents.contains("ROUTE-RUST-1"),
            "rust rule must NOT land in default repo",
        );
        assert!(rust_agents.contains("ROUTE-RUST-1"));
        assert!(
            !rust_agents.contains("ROUTE-UNIV-1"),
            "universal rule must NOT land in rust repo (no override mapped *)",
        );
    }

    #[test]
    fn parse_lock_extracts_id_hash_pairs() {
        let lock = "# header\n\n[[installed]]\nid = \"A-B-1\"\nhash = \"deadbeef\"\n\n[[installed]]\nid = \"C-D-2\"\nhash = \"cafef00d\"\n";
        let pairs = parse_lock(lock);
        assert_eq!(
            pairs,
            vec![
                ("A-B-1".to_string(), "deadbeef".to_string()),
                ("C-D-2".to_string(), "cafef00d".to_string()),
            ]
        );
    }

    #[test]
    fn outdated_reports_changed_and_removed() {
        let current = principle(
            "KEEP-RULE-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the current directive",
        );
        let installed = vec![
            // Same id but a stale hash -> Changed.
            ("KEEP-RULE-1".to_string(), "0000staleHASH000".to_string()),
            // Id no longer in the library -> Removed.
            ("GONE-RULE-1".to_string(), "whatever".to_string()),
            // Custom rules are skipped.
            ("CUSTOM-mine".to_string(), "x".to_string()),
        ];
        let drift = outdated(&installed, std::slice::from_ref(&current));
        assert!(drift.contains(&Drift::Changed("KEEP-RULE-1".to_string())));
        assert!(drift.contains(&Drift::Removed("GONE-RULE-1".to_string())));
        assert_eq!(drift.len(), 2, "custom rule must be skipped");
    }

    #[test]
    fn outdated_is_empty_when_hashes_match() {
        let p = principle(
            "MATCH-RULE-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "body",
        );
        let installed = vec![("MATCH-RULE-1".to_string(), default_principle_hash(&p))];
        assert!(outdated(&installed, std::slice::from_ref(&p)).is_empty());
    }

    #[test]
    fn selections_json_serializes_chosen_option_id() {
        let p = principle(
            "TEST-JSON-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "body",
        );
        let json = selections_json(&[Selection::new(&p)]).expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "TEST-JSON-1");
        assert!(arr[0]["chosen"].is_null(), "unchosen serializes as null");
    }
}
