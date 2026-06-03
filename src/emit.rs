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

/// A principle the user chose. `chosen` is None to take the principle as-written,
/// or Some(alternative) when the user picked one of its alternatives instead.
pub struct Selection<'a> {
    pub principle: &'a Principle,
    pub chosen: Option<String>,
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
}

/// Render the user's resolved decisions as pretty JSON (machine-interchange).
pub fn selections_json(selections: &[Selection]) -> Result<String> {
    let records: Vec<SelectionRecord> = selections
        .iter()
        .map(|s| SelectionRecord {
            principle: s.principle,
            chosen: s.chosen.clone(),
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
///
/// The emitted block represents the project's *adopted* position on this
/// principle, stated as a single directive the consumer agent follows at
/// code-author time. The rule body is either the default summary (when
/// `sel.chosen` is None) or the chosen alternative text (when `sel.chosen`
/// is Some).
///
/// Architect-only fields are deliberately NOT emitted: `alternatives`,
/// `why`, `stance`, `tag`, and the `[choice]` block. Each of those exists
/// to support the architect at curation time (picking which rule to adopt,
/// reviewing rules in PRs, understanding the rule's reasoning), and
/// including them in the consumer agent's input would introduce
/// interpretation surfaces that compete with the directive itself. The
/// consumer agent must see one unambiguous instruction; the emit is tuned
/// for that determinism, not for transparency to a human reading the
/// generated file.
pub fn render(sel: &Selection, scope: Option<&str>) -> String {
    let p = sel.principle;
    let mut s = String::new();
    s.push_str(&format!("### {} — {}\n", p.id, p.title));
    let body = sel.chosen.as_deref().unwrap_or(p.summary.as_str());
    s.push_str(&format!("{body}\n"));
    if let Some(scope) = scope {
        s.push_str(&format!("\n_Applies to:_ `{scope}`\n"));
    }
    s.push('\n');
    s
}

/// A stable hash of an arbitrary string, for the lockfile.
fn content_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// A stable content hash of a principle's meaning, for the lockfile.
/// (std hasher; no crypto dep for v0.1.)
fn principle_hash(p: &Principle) -> String {
    let mut h = DefaultHasher::new();
    p.id.hash(&mut h);
    p.title.hash(&mut h);
    p.summary.hash(&mut h);
    if let Some(w) = &p.why {
        w.hash(&mut h);
    }
    format!("{:016x}", h.finish())
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
pub fn scaffold(out_dir: &Path, selections: &[Selection], custom: &[CustomRule]) -> Result<Outcome> {
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
            let block = render(sel, scope.as_deref());
            let filename = target_filename(&target).to_string();
            buffers.entry(filename.clone()).or_default().push_str(&block);
            *counts.entry(filename).or_insert(0) += 1;
        }

        installed.push((sel.principle.id.clone(), principle_hash(sel.principle)));
    }

    // Append any custom (user-authored) rules, grouped under their domain.
    for c in custom {
        if c.name.trim().is_empty() && c.body.trim().is_empty() {
            continue;
        }
        let domain = if c.domain.is_empty() { "*" } else { c.domain.as_str() };
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
        buffers.entry(filename.clone()).or_default().push_str(&block);
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
        lock.push_str(&format!("[[installed]]\nid = \"{id}\"\nhash = \"{hash}\"\n\n"));
    }
    let lock_path = out_dir.join("camerata.lock");
    fs::write(&lock_path, lock)
        .with_context(|| format!("writing `{}`", lock_path.display()))?;

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
            });
        }
    }
    for c in custom {
        let domain = if c.domain.is_empty() { "*" } else { c.domain.as_str() };
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

    fn principle(
        id: &str,
        title: &str,
        domain: &str,
        layer: Layer,
        enforcement: Enforcement,
        summary: &str,
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
summary = "{summary}"
why = "the architect-only reasoning that must not appear in emitted output"
alternatives = ["the loosened variant that must not appear in emitted output"]
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
        let out = render(&Selection { principle: &p, chosen: None }, None);
        assert!(
            out.starts_with("### TEST-RENDER-1 — the rule title\n"),
            "header missing or malformed; got:\n{out}",
        );
    }

    #[test]
    fn render_uses_default_summary_when_no_alternative_chosen() {
        let p = principle(
            "TEST-DEFAULT-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the default summary directive",
        );
        let out = render(&Selection { principle: &p, chosen: None }, None);
        assert!(out.contains("the default summary directive"));
    }

    #[test]
    fn render_substitutes_chosen_alternative_for_default_summary() {
        let p = principle(
            "TEST-CHOICE-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the default summary directive",
        );
        let out = render(
            &Selection {
                principle: &p,
                chosen: Some("the chosen alternative directive".to_string()),
            },
            None,
        );
        assert!(out.contains("the chosen alternative directive"));
        assert!(
            !out.contains("the default summary directive"),
            "default summary leaked when an alternative was chosen; got:\n{out}",
        );
    }

    #[test]
    fn render_omits_architect_only_fields() {
        // alternatives, why, stance, tag, and choice are architect-only and
        // must never reach the consumer agent. This is the core v0.1 contract.
        let p = principle(
            "TEST-OMIT-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "the directive",
        );
        let out = render(&Selection { principle: &p, chosen: None }, None);
        assert!(!out.contains("architect-only reasoning"), "why leaked");
        assert!(!out.contains("loosened variant"), "alternatives leaked");
        assert!(!out.contains("tag"), "tag literal leaked");
        assert!(!out.contains("stance"), "stance literal leaked");
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
        let out = render(
            &Selection { principle: &p, chosen: None },
            Some("**/*.rs"),
        );
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
        let selections = vec![
            Selection { principle: &prose, chosen: None },
            Selection { principle: &structured, chosen: None },
        ];

        let outcome = scaffold(dir.path(), &selections, &[]).expect("scaffold");
        assert_eq!(outcome.installed, 2);

        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).expect("AGENTS.md exists");
        let conv = fs::read_to_string(dir.path().join("CONVENTIONS.md"))
            .expect("CONVENTIONS.md exists");
        assert!(agents.contains("TEST-PROSE-1"), "prose rule landed in AGENTS.md");
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
            Selection { principle: &framework, chosen: None },
            Selection { principle: &universal, chosen: None },
            Selection { principle: &language, chosen: None },
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
        let selections = vec![Selection { principle: &p, chosen: None }];
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
        let selections = vec![
            Selection { principle: &univ, chosen: None },
            Selection { principle: &rust, chosen: None },
        ];
        let mut overrides: HashMap<String, Vec<PathBuf>> = HashMap::new();
        overrides.insert("rust".to_string(), vec![rust_out.clone()]);

        scaffold_routed(&default_out, &overrides, &selections, &[])
            .expect("scaffold_routed");

        let default_agents =
            fs::read_to_string(default_out.join("AGENTS.md")).expect("default AGENTS.md");
        let rust_agents =
            fs::read_to_string(rust_out.join("AGENTS.md")).expect("rust AGENTS.md");
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
    fn selections_json_omits_keys_for_unchosen_alternatives() {
        let p = principle(
            "TEST-JSON-1",
            "t",
            "*",
            Layer::Universal,
            Enforcement::Prose,
            "body",
        );
        let json =
            selections_json(&[Selection { principle: &p, chosen: None }]).expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "TEST-JSON-1");
        assert!(arr[0]["chosen"].is_null(), "unchosen serializes as null");
    }
}
