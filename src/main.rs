//! camerata CLI — scaffolds AI-orchestration principles.
//!
//! A thin frontend over the `camerata` core library (load -> select -> emit).
//! Tags set the DEFAULT selection state, but nothing is mandatory:
//! universal is on by default yet can be dropped with `--minimal`.

use camerata::emit::{self, Selection};
use camerata::principle::{Principle, Tag};
use camerata::{default_principles_dir, registry};
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
        /// Domain to include: a stack ("rust") or capability ("sql",
        /// "permissions"). Repeatable.
        #[arg(long = "stack", value_name = "DOMAIN")]
        stacks: Vec<String>,
        /// Skip all prompts and take defaults (non-interactive).
        #[arg(long)]
        defaults: bool,
        /// Drop the universal layer too (it is a default, not a requirement).
        #[arg(long)]
        minimal: bool,
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
            json,
        } => cmd_init(&principles, &out, out_domain, stacks, defaults, minimal, json),
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

/// A principle is in scope if it's universal, or its stack was selected.
fn in_scope(p: &Principle, stacks: &[String]) -> bool {
    match p.stack_base() {
        None => true,
        Some(base) => stacks.iter().any(|s| s == base),
    }
}

fn cmd_init(
    principles: &[Principle],
    out: &Path,
    out_domain: Vec<String>,
    mut stacks: Vec<String>,
    defaults: bool,
    minimal: bool,
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
        // Universal is a default, not a requirement: --minimal drops it.
        if minimal && matches!(p.tag, Tag::Universal) {
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
