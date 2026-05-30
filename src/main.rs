//! camerata CLI — scaffolds AI-orchestration principles.
//!
//! A thin frontend over the `camerata` core library (load -> select -> emit).
//! Tags set the DEFAULT selection state, but nothing is mandatory:
//! universal is on by default yet can be dropped with `--minimal`.

use camerata::emit::{self, Selection};
use camerata::principle::{Principle, Tag};
use camerata::{default_principles_dir, is_meta_domain, registry, DEFAULT_SELECTED_DOMAINS};
use anyhow::Result;
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
        } => cmd_init(&principles, &out, out_domain, stacks, defaults, minimal, all, json),
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
                (Some(c), true) => Some(c.default.clone()),
                (Some(c), false) => {
                    Some(inquire::Select::new(&c.prompt, c.options.clone()).prompt()?)
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
