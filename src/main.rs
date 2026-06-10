//! camerata CLI — scaffolds AI-orchestration principles.
//!
//! A thin frontend over the `camerata` core library (load -> select -> emit).
//! Tags set the DEFAULT selection state, but nothing is mandatory:
//! universal is on by default yet can be dropped with `--minimal`.

use anyhow::Result;
use camerata::emit::{self, Selection};
use camerata::principle::{Principle, Tag};
use camerata::{default_principles_dir, is_meta_domain, registry, DEFAULT_SELECTED_DOMAINS};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "camerata",
    version,
    about = "Camerata — scaffold AI-orchestration principles into a repo"
)]
struct Cli {
    /// Directory of principle definitions. Defaults to the bundled library.
    #[arg(long)]
    principles: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List every principle in the library.
    List,
    /// Scaffold selected principles into a target repo.
    Init {
        /// Default directory to write generated files into.
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Per-domain output, e.g. --out-domain rust=../rust-repo. Repeatable;
        /// repeat the same domain to fan it out to multiple repos. Unmapped
        /// domains use --out.
        #[arg(long = "out-domain", value_name = "DOMAIN=PATH")]
        out_domain: Vec<String>,
        /// Domain to include in addition to the default-selected ones
        /// (Universal "*" and "agentic" are always included). A stack
        /// ("rust") or capability ("sql", "permissions"). Repeatable.
        #[arg(long = "stack", value_name = "DOMAIN")]
        stacks: Vec<String>,
        /// Skip all prompts and take defaults (non-interactive).
        #[arg(long)]
        defaults: bool,
        /// Drop the universal layer too (it is a default, not a requirement).
        #[arg(long)]
        minimal: bool,
        /// Include every rule in the selected domains, not just the ones
        /// marked default = true. Use this when you want the opinionated
        /// extras (cache strategies, auto-merge, feature-flag wiring, etc.)
        /// alongside the core defaults.
        #[arg(long)]
        all: bool,
        /// Also write camerata.selections.json (your decisions, machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Print the whole principle library as JSON (catalog export).
    Export,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.principles.unwrap_or_else(default_principles_dir);
    let principles = registry::load_all(&dir)?;

    match cli.command {
        Command::List => cmd_list(&principles),
        Command::Init {
            out,
            out_domain,
            stacks,
            defaults,
            minimal,
            all,
            json,
        } => cmd_init(
            &principles,
            &out,
            out_domain,
            stacks,
            defaults,
            minimal,
            all,
            json,
        ),
        Command::Export => {
            println!("{}", emit::catalog_json(&principles)?);
            Ok(())
        }
    }
}

fn tag_glyph(t: Tag) -> &'static str {
    match t {
        Tag::Universal => "[U]",
        Tag::Stack => "[S]",
        Tag::Choice => "[C]",
    }
}

fn cmd_list(principles: &[Principle]) -> Result<()> {
    println!("{} principle(s) in the library:\n", principles.len());
    for p in principles {
        println!(
            "  {} {:<22} {:<12} {}",
            tag_glyph(p.tag),
            p.id,
            p.domain,
            p.title
        );
    }
    println!("\nlegend:  [U]niversal (default-on)   [S]tack-gated   [C]hoice (prompts you)");
    Ok(())
}

/// A principle is in scope when its domain is part of the curated default-
/// selected set (Universal "*" and "agentic") OR the user listed the domain
/// via `--stack` (matched exactly or by stack-base for nested domains like
/// `rust:seaorm` → base `rust`). Meta-doc domains (`howto`, `contributing`)
/// are never in scope for the CLI scaffold — they're documentation, not
/// adopted conventions.
fn in_scope(p: &Principle, user_domains: &[String]) -> bool {
    if is_meta_domain(p.domain.as_str()) {
        return false;
    }
    if DEFAULT_SELECTED_DOMAINS.contains(&p.domain.as_str()) {
        return true;
    }
    let stack_base = p.stack_base().unwrap_or(p.domain.as_str());
    user_domains
        .iter()
        .any(|d| d == &p.domain || d == stack_base)
}

/// Resolve a picked choice-option LABEL to the directive text that should be
/// emitted as the rule body, returning the value for `Selection.chosen`.
///
/// Returns `None` (emit the summary) when the picked label is the default. For
/// a non-default label, returns `Some(alternative_text)` by matching the label
/// to the alternative at the same position among the non-default options. When
/// the option list and the alternatives list disagree in count (a known data
/// inconsistency in some legacy rules), this falls back to the default and
/// warns on stderr rather than emitting the bare label as a malformed directive.
fn resolve_choice_directive(
    p: &Principle,
    choice: &camerata::principle::Choice,
    picked: &str,
) -> Option<String> {
    if picked == choice.default {
        return None;
    }
    // Position of the picked label among the NON-default options.
    let non_default_index = choice
        .options
        .iter()
        .filter(|o| *o != &choice.default)
        .position(|o| o == picked);
    match non_default_index.and_then(|i| p.alternatives.get(i)) {
        Some(alt) => Some(alt.clone()),
        None => {
            eprintln!(
                "warning: rule {} option \"{}\" has no matching alternative directive; \
                 keeping the default. (Re-author this rule under the decision/options schema.)",
                p.id, picked
            );
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_init(
    principles: &[Principle],
    out: &Path,
    out_domain: Vec<String>,
    mut stacks: Vec<String>,
    defaults: bool,
    minimal: bool,
    include_all: bool,
    json: bool,
) -> Result<()> {
    // Parse per-domain output overrides ("domain=path"); repeat a domain to
    // send it to multiple repos.
    let mut overrides: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for entry in &out_domain {
        match entry.split_once('=') {
            Some((d, p)) if !d.is_empty() && !p.is_empty() => {
                overrides
                    .entry(d.to_string())
                    .or_default()
                    .push(PathBuf::from(p));
            }
            _ => anyhow::bail!("invalid --out-domain `{entry}` (expected DOMAIN=PATH)"),
        }
    }
    // Resolve stacks: prompt if interactive and none were passed.
    if stacks.is_empty() && !defaults {
        let available = registry::available_stacks(principles);
        if !available.is_empty() {
            stacks = inquire::MultiSelect::new(
                "Which stack profiles do you want? (space toggles, enter confirms)",
                available,
            )
            .prompt()?;
        }
    }

    let mut selections: Vec<Selection> = Vec::new();
    for p in principles {
        if !in_scope(p, &stacks) {
            continue;
        }
        // Per-rule default flag gates inclusion within an in-scope domain,
        // matching the GUI behavior. `--all` overrides to include the
        // opinionated extras alongside the core defaults.
        if !p.default && !include_all {
            continue;
        }
        // --minimal additionally drops the universal layer.
        if minimal && p.domain == "*" {
            continue;
        }
        let chosen = match p.tag {
            Tag::Universal | Tag::Stack => None,
            Tag::Choice => match (&p.choice, defaults) {
                // In --defaults mode, "take the default" means use the
                // rule's summary as the emitted body. Setting chosen to the
                // option-label here would make the emit show just the label
                // ("tiered") instead of the prose summary that describes
                // what the default actually entails.
                (Some(_), true) => None,
                (Some(c), false) => {
                    let picked = inquire::Select::new(&c.prompt, c.options.clone()).prompt()?;
                    // Resolve the picked OPTION LABEL to the DIRECTIVE TEXT the
                    // consumer agent must read. Emitting the bare label (the old
                    // behavior) shipped a malformed directive like "per-session
                    // cap only" as the rule body, diverging from the GUI which
                    // emits full alternative text. The default label maps to the
                    // summary (None); a non-default label maps to the alternative
                    // at the same position among the non-default options.
                    resolve_choice_directive(p, c, &picked)
                }
                (None, _) => None,
            },
        };
        selections.push(Selection {
            principle: p,
            chosen,
        });
    }

    let results = emit::scaffold_routed(out, &overrides, &selections, &[])?;
    for (target, outcome) in &results {
        println!(
            "\nScaffolded {} principle(s) into {}:",
            outcome.installed,
            target.display()
        );
        for (file, n) in &outcome.files {
            println!("  + {file}  ({n} rule(s))");
        }
        println!("  + camerata.lock");
    }

    if json {
        let path = out.join("camerata.selections.json");
        std::fs::write(&path, emit::selections_json(&selections)?)?;
        println!("  + camerata.selections.json  (your decisions, machine-readable)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camerata::principle::Choice;

    fn choice_rule() -> Principle {
        let toml_text = r#"
id = "TEST-CHOICE-1"
title = "choice rule"
tag = "choice"
domain = "agentic"
layer = "universal"
enforcement = "prose"
default = true
summary = "the default directive"
why = "reason"
alternatives = [
    "the first alternative directive",
    "the second alternative directive",
]

[choice]
prompt = "pick one"
options = ["default label", "first alt label", "second alt label"]
default = "default label"
"#;
        toml::from_str(toml_text).expect("fixture parses")
    }

    #[test]
    fn resolve_default_label_returns_none() {
        let p = choice_rule();
        let c = p.choice.clone().expect("choice");
        assert_eq!(resolve_choice_directive(&p, &c, "default label"), None);
    }

    #[test]
    fn resolve_non_default_label_returns_matching_alternative_text_not_label() {
        let p = choice_rule();
        let c = p.choice.clone().expect("choice");
        // This is the bug fix: the directive must be the alternative TEXT,
        // never the bare option label.
        assert_eq!(
            resolve_choice_directive(&p, &c, "first alt label"),
            Some("the first alternative directive".to_string()),
        );
        assert_eq!(
            resolve_choice_directive(&p, &c, "second alt label"),
            Some("the second alternative directive".to_string()),
        );
    }

    #[test]
    fn resolve_falls_back_to_default_when_no_matching_alternative() {
        // An option with no corresponding alternative directive (the
        // ORCH-BUDGET-MONITOR-1 "per-session cap only" case) must NOT emit
        // the bare label; it falls back to the default.
        let mut p = choice_rule();
        p.alternatives = vec!["only one alternative".to_string()];
        let c = Choice {
            prompt: "pick".to_string(),
            options: vec![
                "default label".to_string(),
                "first alt label".to_string(),
                "orphan label".to_string(),
            ],
            default: "default label".to_string(),
        };
        // first alt label -> alternatives[0] (ok)
        assert_eq!(
            resolve_choice_directive(&p, &c, "first alt label"),
            Some("only one alternative".to_string()),
        );
        // orphan label -> alternatives[1] missing -> default
        assert_eq!(resolve_choice_directive(&p, &c, "orphan label"), None);
    }
}
