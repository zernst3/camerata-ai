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
        let header = format!(
            "<!-- Generated by camerata. {} principle(s). Edit principle sources, not this file. -->\n\n",
            counts.get(filename).copied().unwrap_or(0)
        );
        fs::write(&path, format!("{header}{body}"))
            .with_context(|| format!("writing `{}`", path.display()))?;
        files.push((filename.clone(), counts.get(filename).copied().unwrap_or(0)));
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
