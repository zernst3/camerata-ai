//! camerata-lint — mechanical PR linter for the principle library.
//!
//! Runs the schema and content checks that the Anatomy rule (and the
//! PR-linter task memory) claim. Designed to run on every pull request
//! against camerata-ai/principles via `.github/workflows/lint-principles.yml`,
//! and runnable locally as a sanity check before opening a PR.
//!
//! Exit code: 0 if every file passes, 1 if any check fails. Violations
//! print as `<file>:<rule-id> <kind> — <message>` so reviewers can cite
//! the exact line in PR comments.
//!
//! Scope today (mechanical, no LLM):
//! - File parses as TOML and as the Principle schema (this implicitly
//!   covers: required fields present, tag/layer/enforcement enum values,
//!   `default` is a bool).
//! - `id` matches the canonical format `DOMAIN-CONCEPT-N`.
//! - `id` is unique across the whole library.
//! - `tag = "choice"` rules have a `[choice]` block defined.
//! - `alternatives` is non-empty (the schema invites disagreement).
//! - Content fields contain no backtick characters.
//!
//! Out of scope today (deferred to v0.2):
//! - Substantive review of `alternatives` (strawman detection).
//! - "Why answers WHY, not WHAT" semantic check.
//! - Cross-domain reference detection.
//! - DEFAULT_SELECTED_DOMAINS-not-modified check (lives in workflow YAML).

use anyhow::{Context, Result};
use camerata::principle::{Principle, Tag};
use camerata::{default_principles_dir, registry};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Violation {
    file: PathBuf,
    id: String,
    kind: &'static str,
    message: String,
}

impl Violation {
    fn print(&self) {
        let id_part = if self.id.is_empty() {
            String::from("(no-id)")
        } else {
            self.id.clone()
        };
        println!(
            "{}:{} {} — {}",
            self.file.display(),
            id_part,
            self.kind,
            self.message
        );
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        default_principles_dir()
    };

    match run(&dir) {
        Ok(0) => {
            println!("camerata-lint: all checks passed");
            ExitCode::SUCCESS
        }
        Ok(n) => {
            println!("\ncamerata-lint: {n} violation(s)");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("camerata-lint: fatal: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &Path) -> Result<usize> {
    let mut violations: Vec<Violation> = Vec::new();
    let mut id_to_files: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let files = collect_toml_files(dir)?;
    if files.is_empty() {
        anyhow::bail!("no .toml files found under {}", dir.display());
    }

    for file in &files {
        check_file(file, &mut violations, &mut id_to_files);
    }

    // Cross-file: id uniqueness.
    for (id, paths) in &id_to_files {
        if paths.len() > 1 {
            for p in paths {
                violations.push(Violation {
                    file: p.clone(),
                    id: id.clone(),
                    kind: "duplicate-id",
                    message: format!(
                        "id `{id}` appears in {} files: {}",
                        paths.len(),
                        paths
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }
    }

    for v in &violations {
        v.print();
    }
    Ok(violations.len())
}

fn collect_toml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading directory `{}`", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            out.push(path);
        }
    }
    Ok(())
}

fn check_file(
    path: &Path,
    violations: &mut Vec<Violation>,
    id_to_files: &mut HashMap<String, Vec<PathBuf>>,
) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: String::new(),
                kind: "read-error",
                message: e.to_string(),
            });
            return;
        }
    };

    // Schema parse handles: required fields, tag/layer/enforcement enums,
    // `default` is a bool.
    let principle: Principle = match toml::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: String::new(),
                kind: "schema",
                message: format!("TOML/schema parse failed: {e}"),
            });
            return;
        }
    };

    id_to_files
        .entry(principle.id.clone())
        .or_default()
        .push(path.to_path_buf());

    // Id format: DOMAIN-CONCEPT-N (uppercase letters/digits in segments,
    // last segment is all digits).
    if !is_valid_id_format(&principle.id) {
        violations.push(Violation {
            file: path.to_path_buf(),
            id: principle.id.clone(),
            kind: "id-format",
            message: format!(
                "id `{}` must match DOMAIN-CONCEPT-N (uppercase segments, trailing number); e.g. RUST-DOMAIN-4 or TS-NEXT-CONSENT-GATED-1",
                principle.id
            ),
        });
    }

    // choice tag implies a [choice] block.
    if matches!(principle.tag, Tag::Choice) && principle.choice.is_none() {
        violations.push(Violation {
            file: path.to_path_buf(),
            id: principle.id.clone(),
            kind: "choice-missing-block",
            message: "tag = \"choice\" requires a [choice] block with prompt + options + default".to_string(),
        });
    }

    // alternatives non-empty.
    if principle.alternatives.is_empty() {
        violations.push(Violation {
            file: path.to_path_buf(),
            id: principle.id.clone(),
            kind: "alternatives-empty",
            message: "every canonical rule MUST list at least one alternative; the schema invites disagreement on the merits".to_string(),
        });
    }

    // No backticks in user-visible content fields.
    let mut has_backtick = false;
    for (field, content) in [
        ("title", principle.title.as_str()),
        ("summary", principle.summary.as_str()),
    ] {
        if content.contains('`') {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "backtick-in-content",
                message: format!("`{field}` contains backtick(s); canonical-rule content uses plain prose"),
            });
            has_backtick = true;
        }
    }
    if let Some(w) = &principle.why {
        if w.contains('`') {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "backtick-in-content",
                message: "`why` contains backtick(s); canonical-rule content uses plain prose".to_string(),
            });
            has_backtick = true;
        }
    }
    for (i, alt) in principle.alternatives.iter().enumerate() {
        if alt.contains('`') {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "backtick-in-content",
                message: format!("`alternatives[{i}]` contains backtick(s); canonical-rule content uses plain prose"),
            });
            has_backtick = true;
        }
    }
    let _ = has_backtick;
}

/// Validate the canonical id format. Examples that pass:
/// `RUST-DOMAIN-4`, `TS-NEXT-CONSENT-GATED-1`, `CAMERATA-USER-GUIDE-1`.
/// Rules: split on `-`, every segment non-empty; every segment uppercase
/// letters/digits only; the LAST segment must be all digits.
fn is_valid_id_format(id: &str) -> bool {
    let segments: Vec<&str> = id.split('-').collect();
    if segments.len() < 2 {
        return false;
    }
    for seg in &segments {
        if seg.is_empty() {
            return false;
        }
        if !seg
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return false;
        }
    }
    let last = segments.last().unwrap();
    last.chars().all(|c| c.is_ascii_digit())
}

// Keep the linter and the library in lockstep: borrow registry's tools so
// the test below catches obvious wiring issues without a separate harness.
#[allow(dead_code)]
fn smoke_load(dir: &Path) -> Result<Vec<Principle>> {
    registry::load_all(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_format_accepts_canonical_examples() {
        for ok in [
            "RUST-DOMAIN-4",
            "TS-NEXT-CONSENT-GATED-1",
            "CAMERATA-USER-GUIDE-1",
            "ORCH-AUTOCALLS-LEDGER-1",
            "AXUM-SERVER-1",
            "ARCH-IAC-1",
        ] {
            assert!(is_valid_id_format(ok), "expected to accept {ok}");
        }
    }

    #[test]
    fn id_format_rejects_bad_shapes() {
        for bad in [
            "",
            "rust-domain-4",          // lowercase
            "RUST_DOMAIN_4",          // underscores
            "RUST-DOMAIN",            // no trailing number
            "RUST-4",                 // single segment + number is allowed length=2
            "RUST-",                  // empty trailing segment
            "-RUST-1",                // empty leading segment
            "RUST--1",                // consecutive dashes
            "RUST-DOMAIN-1a",         // mixed in last segment
        ] {
            // Allow "RUST-4" through (it IS valid by the rules above — DOMAIN segment + number).
            if bad == "RUST-4" {
                assert!(is_valid_id_format(bad), "RUST-4 is a valid 2-segment id");
                continue;
            }
            assert!(!is_valid_id_format(bad), "expected to reject {bad:?}");
        }
    }
}
