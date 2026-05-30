//! Loads every `*.toml` principle definition from a directory tree.
//!
//! Files are organized by domain into subfolders (e.g. `universal/`, `rust/`,
//! `rust/seaorm/`) for human readability; the loader walks recursively and
//! reads the authoritative `domain` field from inside each TOML.

use crate::principle::Principle;
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn load_recursive(dir: &Path, out: &mut Vec<Principle>) -> Result<()> {
    let entries = fs::read_dir(dir)
        .with_context(|| format!("reading principles directory `{}`", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            load_recursive(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading `{}`", path.display()))?;
            let principle: Principle = toml::from_str(&text)
                .with_context(|| format!("parsing `{}`", path.display()))?;
            out.push(principle);
        }
    }
    Ok(())
}

/// Read and parse all principle definitions under `dir` (recursively),
/// sorted by id.
pub fn load_all(dir: &Path) -> Result<Vec<Principle>> {
    let mut principles = Vec::new();
    load_recursive(dir, &mut principles)?;
    principles.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(principles)
}

/// The distinct stack bases present in the library, e.g. {"rust"}.
pub fn available_stacks(principles: &[Principle]) -> Vec<String> {
    let set: BTreeSet<String> = principles
        .iter()
        .filter_map(|p| p.stack_base().map(|s| s.to_string()))
        .collect();
    set.into_iter().collect()
}
