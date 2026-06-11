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
//!   `default` is a bool, the `[decision]` block, the `[[option]]` list).
//! - `id` matches the canonical format `AREA-TOPIC-NUMBER`.
//! - `id` is unique across the whole library.
//! - At least one `[[option]]` (the schema invites disagreement).
//! - Every option has a non-empty id, label, directive, and why.
//! - Option ids are unique within the rule.
//! - `decision.default`, when present, names an existing option id.
//! - Mechanical rules declare a non-empty `qualifies` conformance test.
//! - Content fields contain no backtick characters.
//! - Reports (does not fail) the count of no-default rules.
//!
//! Out of scope today (deferred to v0.2):
//! - Substantive review of options (strawman detection).
//! - "Why answers WHY, not WHAT" semantic check.
//! - Cross-domain reference detection.
//! - DEFAULT_SELECTED_DOMAINS-not-modified check (lives in workflow YAML).

use anyhow::{Context, Result};
use camerata::principle::Principle;
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
    let mut no_default_ids: Vec<String> = Vec::new();

    let files = collect_toml_files(dir)?;
    if files.is_empty() {
        anyhow::bail!("no .toml files found under {}", dir.display());
    }

    for file in &files {
        check_file(file, &mut violations, &mut id_to_files, &mut no_default_ids);
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

    // Report-only: surface no-default (route-to-human) rules. These are valid
    // but should stay rare and deliberate, so make them visible on every run.
    if !no_default_ids.is_empty() {
        no_default_ids.sort();
        println!(
            "note: {} no-default (route-to-human) rule(s): {}",
            no_default_ids.len(),
            no_default_ids.join(", ")
        );
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
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("reading directory `{}`", dir.display()))?;
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
    no_default_ids: &mut Vec<String>,
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
    // `default` is a bool, the [decision] block, and the [[option]] list shape.
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

    // Id format: AREA-TOPIC-NUMBER (at least three uppercase segments,
    // trailing segment all digits).
    if !is_valid_id_format(&principle.id) {
        violations.push(Violation {
            file: path.to_path_buf(),
            id: principle.id.clone(),
            kind: "id-format",
            message: format!(
                "id `{}` must match AREA-TOPIC-NUMBER (at least three uppercase segments, trailing number); e.g. RUST-DOMAIN-4 or UI-CONSENT-GATED-1",
                principle.id
            ),
        });
    }

    // At least one option (the schema invites disagreement on the merits).
    if principle.options.is_empty() {
        violations.push(Violation {
            file: path.to_path_buf(),
            id: principle.id.clone(),
            kind: "options-empty",
            message: "every canonical rule MUST list at least one [[option]]; the schema invites disagreement on the merits".to_string(),
        });
    }

    // Option ids: non-empty fields, unique within the rule.
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, opt) in principle.options.iter().enumerate() {
        if opt.id.trim().is_empty() {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "option-empty-field",
                message: format!("option[{i}] has an empty id"),
            });
        } else if !seen_ids.insert(opt.id.as_str()) {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "option-id-duplicate",
                message: format!("option id `{}` appears more than once in this rule", opt.id),
            });
        }
        for (field, content) in [
            ("label", opt.label.as_str()),
            ("directive", opt.directive.as_str()),
            ("why", opt.why.as_str()),
        ] {
            if content.trim().is_empty() {
                violations.push(Violation {
                    file: path.to_path_buf(),
                    id: principle.id.clone(),
                    kind: "option-empty-field",
                    message: format!("option[{i}] (`{}`) has an empty {field}", opt.id),
                });
            }
        }
    }

    // Mechanical rules MUST carry a `qualifies` conformance test. The field is
    // emitted as the rule's Conformance line and is what makes a mechanical
    // commitment an enforced gate rather than a hollow claim (ORCH-CONFORMANCE-1).
    // prose/structured rules may carry it but it is neither required nor emitted.
    if matches!(
        principle.enforcement,
        camerata::principle::Enforcement::Mechanical
    ) {
        let missing = principle
            .qualifies
            .as_deref()
            .map(|q| q.trim().is_empty())
            .unwrap_or(true);
        if missing {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "qualifies-required-on-mechanical",
                message: "a mechanical rule MUST declare a `qualifies` conformance test (the deterministic check that proves adherence); add prose describing the check or a runnable command".to_string(),
            });
        }
    }

    // decision.default, when present, must name an existing option id.
    match &principle.decision.default {
        Some(def) if principle.option(def).is_none() => {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "default-dangling",
                message: format!(
                    "decision.default `{def}` does not match any [[option]] id in this rule"
                ),
            });
        }
        None => no_default_ids.push(principle.id.clone()),
        _ => {}
    }

    // No backticks in user-visible content fields.
    for (field, content) in [
        ("title", principle.title.as_str()),
        ("decision.question", principle.decision.question.as_str()),
        ("decision.why", principle.decision.why.as_str()),
    ] {
        if content.contains('`') {
            violations.push(Violation {
                file: path.to_path_buf(),
                id: principle.id.clone(),
                kind: "backtick-in-content",
                message: format!(
                    "`{field}` contains backtick(s); canonical-rule content uses plain prose"
                ),
            });
        }
    }
    for (i, opt) in principle.options.iter().enumerate() {
        for (field, content) in [
            ("label", opt.label.as_str()),
            ("directive", opt.directive.as_str()),
            ("why", opt.why.as_str()),
        ] {
            if content.contains('`') {
                violations.push(Violation {
                    file: path.to_path_buf(),
                    id: principle.id.clone(),
                    kind: "backtick-in-content",
                    message: format!("option[{i}].{field} contains backtick(s); canonical-rule content uses plain prose"),
                });
            }
        }
    }
}

/// Validate the canonical id format: AREA-TOPIC-NUMBER. Examples that pass:
/// `RUST-DOMAIN-4`, `TS-NEXT-CONSENT-GATED-1`, `CAMERATA-USER-GUIDE-1`.
/// Rules: split on `-`, at least THREE segments, every segment non-empty,
/// every segment uppercase letters/digits only, the LAST segment all digits.
///
/// The three-segment floor is what carries the AREA-TOPIC-NUMBER convention:
/// AREA names the cluster (RUST, ARCH, ORCH, ...), TOPIC names the concept
/// inside it (DOMAIN, CONTEXT-OVERRIDE, ...), and NUMBER disambiguates
/// successive rules in the same topic. Two-segment ids like `RUST-4` collapse
/// AREA and TOPIC, which the library has never used and the format rejects.
fn is_valid_id_format(id: &str) -> bool {
    let segments: Vec<&str> = id.split('-').collect();
    if segments.len() < 3 {
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

    /// A valid new-schema rule body with a customizable id. Used as the clean
    /// base that individual tests mutate to trigger a single violation.
    fn valid_rule(id: &str) -> String {
        format!(
            r#"
id = "{id}"
title = "a clean rule"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "how is the thing done?"
default = "primary"
why = "the reason this decision matters"

[[option]]
id = "primary"
label = "the canonical option"
directive = "the directive body"
why = "the canonical option is correct here"

[[option]]
id = "alt"
label = "the loosened option"
directive = "the loosened variant"
why = "looser; defensible only in narrow contexts"
"#
        )
    }

    #[test]
    fn id_format_accepts_canonical_examples() {
        for ok in [
            "RUST-DOMAIN-4",
            "UI-CONSENT-GATED-1",
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
            "rust-domain-4",  // lowercase
            "RUST_DOMAIN_4",  // underscores
            "RUST-DOMAIN",    // no trailing number
            "RUST-4",         // only two segments (AREA-NUMBER, no TOPIC)
            "RUST-DOMAIN-",   // empty trailing segment
            "-RUST-DOMAIN-1", // empty leading segment
            "RUST--DOMAIN-1", // consecutive dashes
            "RUST-DOMAIN-1a", // mixed in last segment
        ] {
            assert!(!is_valid_id_format(bad), "expected to reject {bad:?}");
        }
    }

    #[test]
    fn id_format_requires_at_least_three_segments() {
        assert!(!is_valid_id_format("RUST-4"));
        assert!(is_valid_id_format("RUST-DOMAIN-4"));
    }

    /// Write `text` into a tempdir as `rule.toml`, run check_file on it, and
    /// return the kinds of violations that fired.
    fn check_one(text: &str) -> (tempfile::TempDir, Vec<&'static str>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("rule.toml");
        std::fs::write(&path, text).expect("write");
        let mut violations = Vec::new();
        let mut id_to_files = HashMap::new();
        let mut no_default_ids = Vec::new();
        check_file(
            &path,
            &mut violations,
            &mut id_to_files,
            &mut no_default_ids,
        );
        let kinds: Vec<&'static str> = violations.iter().map(|v| v.kind).collect();
        (dir, kinds)
    }

    #[test]
    fn schema_violation_for_malformed_toml() {
        let (_d, kinds) = check_one("this is not = valid toml [[[");
        assert!(
            kinds.contains(&"schema"),
            "expected schema violation; got {kinds:?}",
        );
    }

    #[test]
    fn schema_violation_for_missing_decision_block() {
        // Missing the required [decision] block.
        let text = r#"
id = "TEST-MISSING-1"
title = "missing decision"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"schema"),
            "expected schema violation; got {kinds:?}",
        );
    }

    #[test]
    fn id_format_violation_emitted_when_id_is_lowercase() {
        let text = valid_rule("rust-domain-4");
        let (_d, kinds) = check_one(&text);
        assert!(
            kinds.contains(&"id-format"),
            "expected id-format violation; got {kinds:?}",
        );
    }

    #[test]
    fn empty_options_list_is_flagged() {
        let text = r#"
id = "TEST-NOOPT-1"
title = "no options"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"options-empty"),
            "expected options-empty; got {kinds:?}",
        );
    }

    #[test]
    fn dangling_default_is_flagged() {
        let text = r#"
id = "TEST-DANGLE-1"
title = "dangling default"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
default = "nonexistent"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"default-dangling"),
            "expected default-dangling; got {kinds:?}",
        );
    }

    #[test]
    fn duplicate_option_id_within_rule_is_flagged() {
        let text = r#"
id = "TEST-DUPOPT-1"
title = "dup option id"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
default = "a"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "w"

[[option]]
id = "a"
label = "a again"
directive = "do a differently"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"option-id-duplicate"),
            "expected option-id-duplicate; got {kinds:?}",
        );
    }

    #[test]
    fn empty_option_directive_is_flagged() {
        let text = r#"
id = "TEST-EMPTYDIR-1"
title = "empty directive"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
default = "a"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "   "
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"option-empty-field"),
            "expected option-empty-field; got {kinds:?}",
        );
    }

    #[test]
    fn no_default_rule_is_collected_not_flagged() {
        // A rule with no decision.default is valid (route-to-human); it must
        // not produce a violation, only get reported.
        let text = r#"
id = "TEST-NODEF-1"
title = "open decision"
tag = "universal"
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
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("rule.toml");
        std::fs::write(&path, text).expect("write");
        let mut violations = Vec::new();
        let mut id_to_files = HashMap::new();
        let mut no_default_ids = Vec::new();
        check_file(
            &path,
            &mut violations,
            &mut id_to_files,
            &mut no_default_ids,
        );
        assert!(violations.is_empty(), "no-default rule should not violate");
        assert_eq!(no_default_ids, vec!["TEST-NODEF-1".to_string()]);
    }

    #[test]
    fn mechanical_rule_without_qualifies_is_flagged() {
        let text = r#"
id = "TEST-MECH-NOQUAL-1"
title = "a mechanical rule missing its conformance test"
tag = "universal"
layer = "universal"
enforcement = "mechanical"
default = true

[decision]
question = "q"
default = "a"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"qualifies-required-on-mechanical"),
            "expected qualifies-required-on-mechanical; got {kinds:?}",
        );
    }

    #[test]
    fn mechanical_rule_with_qualifies_passes() {
        let text = r#"
id = "TEST-MECH-QUAL-1"
title = "a mechanical rule with its conformance test"
tag = "universal"
layer = "universal"
enforcement = "mechanical"
default = true
qualifies = "a clippy lint fails the build if the forbidden pattern appears"

[decision]
question = "q"
default = "a"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "do a"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            !kinds.contains(&"qualifies-required-on-mechanical"),
            "mechanical rule with qualifies should not flag; got {kinds:?}",
        );
    }

    #[test]
    fn structured_rule_without_qualifies_is_fine() {
        // The requirement is mechanical-only; a structured rule needs no qualifies.
        let text = valid_rule("TEST-STRUCT-NOQUAL-1")
            .replace(r#"enforcement = "prose""#, r#"enforcement = "structured""#);
        let (_d, kinds) = check_one(&text);
        assert!(
            !kinds.contains(&"qualifies-required-on-mechanical"),
            "structured rule must not require qualifies; got {kinds:?}",
        );
    }

    #[test]
    fn backtick_in_directive_is_flagged() {
        let text = r#"
id = "TEST-BACKTICK-1"
title = "ok title"
tag = "universal"
layer = "universal"
enforcement = "prose"
default = true

[decision]
question = "q"
default = "a"
why = "w"

[[option]]
id = "a"
label = "a"
directive = "body with a `backtick` inside"
why = "w"
"#;
        let (_d, kinds) = check_one(text);
        assert!(
            kinds.contains(&"backtick-in-content"),
            "expected backtick-in-content for directive; got {kinds:?}",
        );
    }

    #[test]
    fn backtick_in_title_is_flagged() {
        let text = valid_rule("TEST-BACKTICK-2").replace(
            r#"title = "a clean rule""#,
            r#"title = "bad `title` with backticks""#,
        );
        let (_d, kinds) = check_one(&text);
        assert!(
            kinds.contains(&"backtick-in-content"),
            "expected backtick-in-content for title; got {kinds:?}",
        );
    }

    #[test]
    fn backtick_in_decision_why_is_flagged() {
        let text = valid_rule("TEST-BACKTICK-3").replace(
            r#"why = "the reason this decision matters""#,
            r#"why = "reason with a `backtick`""#,
        );
        let (_d, kinds) = check_one(&text);
        assert!(
            kinds.contains(&"backtick-in-content"),
            "expected backtick-in-content for decision.why; got {kinds:?}",
        );
    }

    #[test]
    fn clean_rule_produces_no_violations() {
        let (_d, kinds) = check_one(&valid_rule("TEST-CLEAN-1"));
        assert!(
            kinds.is_empty(),
            "clean rule produced violations: {kinds:?}"
        );
    }

    #[test]
    fn duplicate_id_across_files_is_flagged_by_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("one.toml"), valid_rule("TEST-DUP-1")).expect("write one");
        std::fs::write(dir.path().join("two.toml"), valid_rule("TEST-DUP-1")).expect("write two");

        let violation_count = run(dir.path()).expect("run");
        assert!(
            violation_count >= 2,
            "expected at least 2 duplicate-id violations; got {violation_count}",
        );
    }
}
