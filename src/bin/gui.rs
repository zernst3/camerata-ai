//! camerata GUI (preview) — a Dioxus Desktop frontend over the core lib.
//!
//! Dioxus Desktop renders via the OS webview (lightweight, no bundled Chromium)
//! and is cross-platform (Windows/macOS/Linux). The same component tree can also
//! target WebAssembly for an in-browser build later.
//!
//! Shows the curation UX: collapsible domain groups, per-principle checkboxes
//! (universal pre-checked but deselectable), a detail pane with rationale +
//! SELECTABLE alternatives, free-text custom rules attachable to any domain,
//! and Generate / Export-JSON actions.

use camerata::emit::{self, CustomRule, Selection};
use camerata::principle::{Principle, Tag};
use camerata::profile::{Profile, PROFILE_VERSION};
use camerata::{
    default_principles_dir, domain_label, is_meta_domain, registry, DEFAULT_SELECTED_DOMAINS,
};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn main() {
    // LastWindowHides keeps the runtime alive when the OS close button is
    // clicked, so we can intercept the close and prompt the user to save.
    // The window is auto-hidden by the runtime after our wry handler runs;
    // a use_effect in app() re-shows it so the prompt is visible.
    let cfg = dioxus::desktop::Config::new()
        .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::LastWindowHides);
    LaunchBuilder::desktop().with_cfg(cfg).launch(app);
}

/// A parsed block from a principle's summary text. The renderer in the
/// right pane converts each block into an appropriate element so the user
/// guide (and any other principle that uses these markers) can have
/// headings, paragraphs, and bullet lists. Plain prose still renders as
/// paragraphs, so existing principles are unaffected.
enum Block {
    H1(String),
    H2(String),
    Para(String),
    List(Vec<String>),
}

/// What the user has asked to delete. The deletion is gated on a confirm
/// banner so accidental clicks on the × button don't silently destroy work.
#[derive(Clone)]
enum DeleteTarget {
    /// Index into the `custom_rules` Vec.
    CustomRule(usize),
    /// Domain name to delete, along with every custom rule scoped to it.
    CustomDomain(String),
}

/// A navigation the user requested while editing an in-progress custom rule.
/// When the form is dirty, the navigation is deferred and a save-or-discard
/// prompt appears; the chosen action runs the corresponding side effect
/// before the navigation completes.
#[derive(Clone)]
enum PendingNav {
    /// Switch the form to view/edit a different custom rule by index.
    OpenCustomRule(usize),
    /// Show a canonical principle's detail.
    OpenCanonical(usize),
    /// Open the "add custom rule" form for this domain.
    AddCustomRule(String),
    /// Open the "add custom domain" form.
    AddCustomDomain,
    /// Open the rename-custom-domain form for the given old name.
    RenameCustomDomain(String),
}

/// Markdown-lite parser: recognizes a small subset that's enough for a
/// readable user guide without pulling in a full markdown crate.
///
/// Rules:
/// - Line starting with `# ` -> H1
/// - Line starting with `## ` -> H2
/// - Consecutive lines starting with `- ` -> a List with each as an item
/// - Blank line -> separator (closes any open list)
/// - Anything else -> Para
fn parse_markdown_lite(text: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut current_list: Vec<String> = Vec::new();
    let flush_list = |list: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !list.is_empty() {
            blocks.push(Block::List(std::mem::take(list)));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_list(&mut current_list, &mut blocks);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            current_list.push(rest.to_string());
            continue;
        }
        flush_list(&mut current_list, &mut blocks);
        if let Some(rest) = trimmed.strip_prefix("## ") {
            blocks.push(Block::H2(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            blocks.push(Block::H1(rest.to_string()));
        } else {
            blocks.push(Block::Para(trimmed.to_string()));
        }
    }
    flush_list(&mut current_list, &mut blocks);
    blocks
}

/// Resolve a deferred navigation after the user has answered the save-or-
/// discard prompt. Clears all right-pane modes, then opens the requested
/// target. Signals are passed by value because Dioxus Signal<T> is Copy.
#[allow(clippy::too_many_arguments)]
fn apply_pending_nav(
    nav: Option<PendingNav>,
    mut custom_name: Signal<String>,
    mut custom_body: Signal<String>,
    mut editing_custom_idx: Signal<Option<usize>>,
    mut editing_custom_domain: Signal<Option<String>>,
    mut renamed_domain_name: Signal<String>,
    mut adding_custom: Signal<Option<String>>,
    mut adding_domain: Signal<bool>,
    mut new_domain_name: Signal<String>,
    mut detail: Signal<Option<usize>>,
    mut custom_form_dirty: Signal<bool>,
    custom_rules: &[CustomRule],
) {
    let Some(nav) = nav else { return };
    // Clear all right-pane modes first; the matching arm below sets the
    // new state for the requested target.
    custom_name.set(String::new());
    custom_body.set(String::new());
    custom_form_dirty.set(false);
    editing_custom_idx.set(None);
    editing_custom_domain.set(None);
    adding_custom.set(None);
    adding_domain.set(false);
    detail.set(None);
    match nav {
        PendingNav::OpenCustomRule(idx) => {
            if let Some(rule) = custom_rules.get(idx) {
                custom_name.set(rule.name.clone());
                custom_body.set(rule.body.clone());
                editing_custom_idx.set(Some(idx));
            }
        }
        PendingNav::OpenCanonical(i) => {
            detail.set(Some(i));
        }
        PendingNav::AddCustomRule(d) => {
            adding_custom.set(Some(d));
        }
        PendingNav::AddCustomDomain => {
            adding_domain.set(true);
            new_domain_name.set(String::new());
        }
        PendingNav::RenameCustomDomain(d) => {
            renamed_domain_name.set(d.clone());
            editing_custom_domain.set(Some(d));
        }
    }
}

/// Cross-platform autosave path: `<data_local_dir>/camerata/in-progress.json`.
/// Returns None if the platform data-local dir cannot be determined.
fn autosave_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("camerata").join("in-progress.json"))
}

/// Run the scaffold emit and return either a success status string or an
/// error string. Pure function over the current state so the same logic can
/// be invoked from multiple buttons (Generate-confirm banner offers Save-and-
/// generate vs. Generate-without-saving paths).
#[allow(clippy::too_many_arguments)]
fn run_generate(
    principles: &[Principle],
    selected: &[bool],
    chosen: &[Option<String>],
    custom_rules: &[CustomRule],
    out_dir: &str,
    repos: &[String],
    domain_repos: &HashMap<String, HashSet<String>>,
) -> Result<String, String> {
    let sels: Vec<Selection> = principles
        .iter()
        .enumerate()
        .filter(|(i, _)| *selected.get(*i).unwrap_or(&false))
        .map(|(i, p)| Selection {
            principle: p,
            chosen: chosen.get(i).cloned().flatten(),
        })
        .collect();
    let repo_set: HashSet<String> = repos.iter().cloned().collect();
    let overrides: HashMap<String, Vec<PathBuf>> = domain_repos
        .iter()
        .map(|(k, set)| {
            let paths: Vec<PathBuf> = set
                .iter()
                .filter(|p| repo_set.contains(*p))
                .map(PathBuf::from)
                .collect();
            (k.clone(), paths)
        })
        .filter(|(_, v)| !v.is_empty())
        .collect();
    match emit::scaffold_routed(std::path::Path::new(out_dir), &overrides, &sels, custom_rules) {
        Ok(results) => {
            let targets: Vec<String> = results
                .iter()
                .map(|(t, o)| format!("{} ({} rules)", t.display(), o.installed))
                .collect();
            Ok(format!("Generated into {}", targets.join("; ")))
        }
        Err(e) => Err(format!("Error: {e}")),
    }
}

/// Best-effort delete the autosave temp file. Called after Generate so the
/// next launch starts clean rather than offering recovery of a state that
/// has already been emitted.
fn clear_autosave() {
    if let Some(path) = autosave_path() {
        let _ = std::fs::remove_file(&path);
    }
}

/// Build a Profile snapshot from the current GUI signal state.
#[allow(clippy::too_many_arguments)]
fn snapshot_profile(
    principles: &[Principle],
    selected: &[bool],
    selected_domains: &HashSet<String>,
    chosen: &[Option<String>],
    custom_alts: &[Vec<String>],
    custom_rules: &[CustomRule],
    custom_domains: &[String],
    out_dir: &str,
    repos: &[String],
    domain_repos: &HashMap<String, HashSet<String>>,
) -> Profile {
    let mut selected_ids = Vec::new();
    let mut chosen_map = HashMap::new();
    let mut custom_alts_map = HashMap::new();
    for (i, p) in principles.iter().enumerate() {
        if selected.get(i).copied().unwrap_or(false) {
            selected_ids.push(p.id.clone());
        }
        if let Some(v) = chosen.get(i).and_then(|c| c.as_ref()) {
            chosen_map.insert(p.id.clone(), v.clone());
        }
        let alts = custom_alts.get(i).cloned().unwrap_or_default();
        if !alts.is_empty() {
            custom_alts_map.insert(p.id.clone(), alts);
        }
    }
    let repos_map: HashMap<String, Vec<String>> = domain_repos
        .iter()
        .map(|(k, set)| (k.clone(), set.iter().cloned().collect()))
        .collect();
    Profile {
        version: PROFILE_VERSION,
        selected_ids,
        selected_domains: selected_domains.iter().cloned().collect(),
        chosen: chosen_map,
        custom_alternatives: custom_alts_map,
        custom_rules: custom_rules.to_vec(),
        custom_domains: custom_domains.to_vec(),
        out_dir: out_dir.to_string(),
        repos: repos.to_vec(),
        domain_repos: repos_map,
    }
}

fn tag_glyph(t: Tag) -> &'static str {
    match t {
        Tag::Universal => "[U]",
        Tag::Stack => "[S]",
        Tag::Choice => "[C]",
    }
}

fn opt_style(selected: bool) -> String {
    let base = "display:block; width:100%; box-sizing:border-box; text-align:left; margin:3px 0; padding:6px 8px; \
                border:1px solid #ccc; border-radius:4px; cursor:pointer;";
    if selected {
        format!("{base} background:#dce9ff; border-color:#5b8def;")
    } else {
        format!("{base} background:#fff;")
    }
}

fn app() -> Element {
    // Library loaded once, sorted for grouped display.
    //
    // Sort buckets, in display order:
    //   1. Universal ("*")
    //   2. Capability domains (sql, ui, permissions, ...) — alphabetical
    //   3. Stack profiles — grouped by stack_base so a language and its
    //      nested library/framework domains cluster (rust + rust:dioxus +
    //      rust:seaorm together; js + js:next together)
    //
    // Within a stack family, the layer order (Language < Library <
    // Framework) determines internal order so a language-layer profile
    // header sits above its library and framework children.
    //
    // The meta domains (howto, contributing) fall into the stack bucket
    // here because they don't appear in CAPABILITIES; pinning to the top
    // happens later when the groups list is built.
    let principles = use_signal(|| {
        let mut p = registry::load_all(&default_principles_dir()).unwrap_or_default();
        let bucket = |d: &str| -> u8 {
            if d == "*" {
                0
            } else if camerata::is_capability(d) {
                1
            } else {
                2
            }
        };
        p.sort_by(|a, b| {
            let ab = bucket(a.domain.as_str());
            let bb = bucket(b.domain.as_str());
            ab.cmp(&bb).then_with(|| {
                if ab == 2 {
                    // Stack bucket: cluster by stack_base, then layer, then
                    // exact domain, then id.
                    let a_base = a.stack_base().unwrap_or(a.domain.as_str());
                    let b_base = b.stack_base().unwrap_or(b.domain.as_str());
                    a_base
                        .cmp(b_base)
                        .then(a.layer.cmp(&b.layer))
                        .then(a.domain.cmp(&b.domain))
                        .then(a.id.cmp(&b.id))
                } else {
                    // Universal or capability bucket: by domain then id.
                    a.domain.cmp(&b.domain).then(a.id.cmp(&b.id))
                }
            })
        });
        p
    });

    // Currently-selected domains. Initialized to the curated default set
    // (Universal + Agentic). Per-rule selection is gated by membership in
    // this set: a rule's checkbox can only be toggled when its domain is
    // here, and a rule's initial checked state requires both membership
    // here and the per-rule `default` flag.
    let mut selected_domains = use_signal(|| {
        let mut s = HashSet::new();
        for d in DEFAULT_SELECTED_DOMAINS {
            s.insert(d.to_string());
        }
        s
    });

    // Per-principle state (parallel to `principles`).
    // Initial: checked iff the rule's domain is currently selected AND the
    // rule's `default` flag is true.
    let mut selected = use_signal(|| {
        let domains: HashSet<&str> = DEFAULT_SELECTED_DOMAINS.iter().copied().collect();
        principles
            .read()
            .iter()
            .map(|p| domains.contains(p.domain.as_str()) && p.default)
            .collect::<Vec<bool>>()
    });
    let mut chosen = use_signal(|| {
        (0..principles.read().len())
            .map(|_| None::<String>)
            .collect::<Vec<Option<String>>>()
    });
    // User-authored extra alternatives per built-in principle (parallel index).
    let mut custom_alts = use_signal(|| {
        (0..principles.read().len())
            .map(|_| Vec::<String>::new())
            .collect::<Vec<Vec<String>>>()
    });
    let mut new_alt = use_signal(String::new);

    let mut detail = use_signal(|| None::<usize>);
    let mut expanded = use_signal(|| {
        let mut s = HashSet::new();
        s.insert("*".to_string());
        s.insert("howto".to_string());
        s.insert("contributing".to_string());
        s
    });
    let mut custom_rules = use_signal(Vec::<CustomRule>::new);
    let mut custom_name = use_signal(String::new);
    let mut custom_body = use_signal(String::new);
    // Some(domain) while the custom-rule form is open for that domain.
    let mut adding_custom = use_signal(|| None::<String>);
    // Some(idx) while the user is editing an existing custom rule (by index
    // into `custom_rules`). The form is the same shape as the create form;
    // the difference is that Save updates the existing entry in place
    // rather than pushing a new one.
    let mut editing_custom_idx = use_signal(|| None::<usize>);
    // Some(original_name) while the user is renaming an existing custom
    // domain. The form is a single name field; Save propagates the new
    // name to every place the old name was referenced (custom_rules entries
    // scoped to it, selected_domains, expanded, domain_repos).
    let mut editing_custom_domain = use_signal(|| None::<String>);
    let mut renamed_domain_name = use_signal(String::new);
    // True when the open custom-rule form has unsaved edits relative to the
    // version loaded into the form. Drives whether Save/Cancel are visible
    // (view-only mode shows neither) and whether click-away triggers a
    // save-or-discard prompt.
    let mut custom_form_dirty = use_signal(|| false);
    // When the user tries to navigate away from a dirty custom-rule form,
    // the target action is captured here instead of running immediately.
    // The prompt then offers Save / Discard / Cancel. Save and Discard both
    // execute the pending action; Cancel keeps the user in the form.
    let mut pending_nav = use_signal(|| None::<PendingNav>);
    let mut out_dir = use_signal(|| "/tmp/camerata-demo".to_string());
    // Target repos the architect adds, plus which repos each domain lands in.
    let mut repos = use_signal(Vec::<String>::new);
    let mut new_repo = use_signal(String::new);
    let mut domain_repos = use_signal(HashMap::<String, HashSet<String>>::new);
    let mut show_targets = use_signal(|| false);
    let mut status = use_signal(String::new);

    // User-created custom domains (no canonical principles; only custom rules).
    let mut custom_domains = use_signal(Vec::<String>::new);
    // True while the "add custom domain" modal is open.
    let mut adding_domain = use_signal(|| false);
    let mut new_domain_name = use_signal(String::new);

    // Recovery state: on launch, check the autosave path. If a profile exists
    // there, hold it in pending_recovery and surface the recovery banner.
    let mut pending_recovery = use_signal(|| {
        autosave_path()
            .and_then(|p| if p.exists() { Profile::load(&p).ok() } else { None })
    });
    // Soft-warning banner for selected ids in a loaded profile that don't
    // exist in the current library (renamed/removed canonical rules).
    let mut missing_ids_warning = use_signal(Vec::<String>::new);
    // Tracks whether the autosave use_effect has fired at least once. The
    // first fire happens at component mount with the initial state — we
    // skip writing the autosave then so a fresh launch with no user activity
    // doesn't create a misleading recovery banner next time. Subsequent
    // fires correspond to real state changes and DO write.
    let mut autosave_initialized = use_signal(|| false);

    // True while the Generate-confirm banner is showing (between the user
    // clicking Generate and choosing Save-and-generate / Generate-without-
    // saving / Cancel).
    let mut pending_generate = use_signal(|| false);

    // True while the exit-confirm banner is showing. Triggered by the
    // action-bar Exit button OR by the OS window close button (see wry
    // event handler below).
    let mut exit_prompt_open = use_signal(|| false);

    // The deletion the user has requested but not yet confirmed. When Some,
    // the delete-confirm banner shows and the corresponding × button click
    // is what set it. Cleared on Confirm (after deletion) or Cancel.
    let mut pending_delete = use_signal(|| None::<DeleteTarget>);

    // Handle to the OS window. Used to (a) re-show the window after the
    // runtime auto-hides it on close, and (b) actually close the window
    // when the user picks Save-and-exit or Discard-and-exit.
    let window_ctx = dioxus::desktop::use_window();

    // Intercept the OS window close button. The wry handler runs before
    // the runtime's LastWindowHides behavior, so we set the prompt flag
    // here; the use_effect below re-shows the window after the auto-hide.
    {
        use dioxus::desktop::tao::event::{Event, WindowEvent as TaoWindowEvent};
        dioxus::desktop::use_wry_event_handler(move |event, _target| {
            if let Event::WindowEvent { event: TaoWindowEvent::CloseRequested, .. } = event {
                exit_prompt_open.set(true);
            }
        });
    }

    // Re-show the window when the exit prompt opens. The runtime hides the
    // window after a CloseRequested event (because of LastWindowHides), so
    // without this the prompt would be invisible to the user.
    {
        let window_ctx = window_ctx.clone();
        use_effect(move || {
            if exit_prompt_open() {
                window_ctx.window.set_visible(true);
            }
        });
    }

    // Build display groups (domain -> rows), preserving sorted order.
    let plist = principles.read().clone();
    let exp = expanded.read().clone();
    let mut groups: Vec<(String, bool, Vec<(usize, String, &'static str)>)> = Vec::new();
    for (i, p) in plist.iter().enumerate() {
        let row = (i, p.title.clone(), tag_glyph(p.tag));
        match groups.last_mut() {
            Some((d, _, rows)) if *d == p.domain => rows.push(row),
            _ => groups.push((p.domain.clone(), exp.contains(&p.domain), vec![row])),
        }
    }
    // Pin the meta-documentation groups to the top. Order matters: pin
    // `contributing` first so it lands at position 0, then pin `howto` so
    // it displaces `contributing` to position 1 and sits above it. Final
    // order: howto, contributing, then everything else.
    if let Some(pos) = groups.iter().position(|(d, _, _)| d == "contributing") {
        let g = groups.remove(pos);
        groups.insert(0, g);
    }
    if let Some(pos) = groups.iter().position(|(d, _, _)| d == "howto") {
        let g = groups.remove(pos);
        groups.insert(0, g);
    }
    // Append synthetic empty groups for user-created custom domains. They
    // hold only custom rules (no canonical principles).
    for cd in custom_domains.read().iter() {
        if !groups.iter().any(|(d, _, _)| d == cd) {
            groups.push((cd.clone(), exp.contains(cd), Vec::new()));
        }
    }
    let selected_count = selected.read().iter().filter(|b| **b).count();
    let detail_idx = detail();

    // Autosave: write a fresh snapshot to the autosave path whenever any of
    // the tracked signals change. Skip the first fire (component mount) so
    // an idle launch doesn't create a misleading recovery file.
    //
    // Note: autosave_initialized is read with `peek()` so the effect does NOT
    // subscribe to it. Reading it via the normal `.read()` / call-syntax
    // would create a read-write-same-signal pattern inside the effect,
    // which Dioxus correctly warns about as a potential infinite-loop
    // shape. `peek()` reads the current value without subscribing, so the
    // set() below only triggers a re-fire if some OTHER subscribed signal
    // also changed — which is exactly the lifecycle we want.
    use_effect(move || {
        let snap = snapshot_profile(
            &principles.read(),
            &selected.read(),
            &selected_domains.read(),
            &chosen.read(),
            &custom_alts.read(),
            &custom_rules.read(),
            &custom_domains.read(),
            &out_dir.read(),
            &repos.read(),
            &domain_repos.read(),
        );
        if !*autosave_initialized.peek() {
            autosave_initialized.set(true);
            return;
        }
        // Critical: while the recovery banner is up, the in-memory state is
        // the fresh-launch default (NOT the recovery file's content). If we
        // wrote the autosave here, we would silently overwrite the recovery
        // file with the defaults and destroy the data the user was about to
        // resume. Skip writes until the user resolves the banner (Resume
        // loads the file's state, after which writes can resume; Start over
        // deletes the file, after which writes recreate it from current
        // state). Peek so this guard does NOT re-trigger the effect when
        // pending_recovery changes.
        if pending_recovery.peek().is_some() {
            return;
        }
        if let Some(path) = autosave_path() {
            let _ = snap.save(&path);
        }
    });

    rsx! {
        div { style: "font-family: -apple-system, system-ui, sans-serif; padding: 12px; color:#222;",
            h2 { style: "margin:0 0 8px 0;",
                "camerata "
                span { style: "font-weight:400; color:#777; font-size:0.7em;",
                    "· AI-orchestration scaffolder (preview)"
                }
            }

            // Recovery modal: when an autosave file exists from a prior
            // session, force the user to decide before doing anything else.
            // The dimmed full-window overlay blocks clicks from reaching the
            // rest of the app, and the modal sits centered on top. The user
            // must pick Resume (restore prior state) or Start over (discard).
            // No outside-click dismiss — silent dismissal would leave the
            // user in an ambiguous "did I resume or not?" state.
            if pending_recovery.read().is_some() {
                div { style: "position:fixed; top:0; left:0; right:0; bottom:0; background:rgba(20,30,50,0.55); z-index:1000; display:flex; align-items:center; justify-content:center;",
                    div { style: "background:#fff; border:1px solid #d0d4dc; border-radius:10px; box-shadow:0 12px 40px rgba(0,0,0,0.28); padding:24px 28px; max-width:520px; min-width:380px;",
                        div { style: "font-weight:700; font-size:1.15em; margin-bottom:10px; color:#222;",
                            "Resume your previous session?"
                        }
                        div { style: "color:#555; line-height:1.5; margin-bottom:18px;",
                            "An unsaved in-progress profile from a previous session was found. Pick up where you left off, or start over with the defaults. The rest of the app is locked until you choose."
                        }
                        div { style: "display:flex; gap:10px; justify-content:flex-end;",
                            button {
                                style: "background:#fff; border:1px solid #aab; border-radius:6px; padding:7px 14px; cursor:pointer;",
                                onclick: move |_| {
                                    if let Some(path) = autosave_path() {
                                        let _ = std::fs::remove_file(&path);
                                    }
                                    pending_recovery.set(None);
                                },
                                "Start over"
                            }
                            button {
                                style: "background:#1452a3; color:#fff; border:none; border-radius:6px; padding:7px 16px; cursor:pointer; font-weight:600;",
                                onclick: move |_| {
                                    let prof_opt = pending_recovery.read().clone();
                                    if let Some(prof) = prof_opt {
                                        let lib_ids: Vec<String> = principles.read().iter().map(|p| p.id.clone()).collect();
                                        let missing = prof.missing_ids(lib_ids.iter().map(|s| s.as_str()));
                                        let lib = principles.read().clone();
                                        let id_to_idx: HashMap<String, usize> = lib.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
                                        let mut new_selected = vec![false; lib.len()];
                                        for id in &prof.selected_ids {
                                            if let Some(&i) = id_to_idx.get(id) { new_selected[i] = true; }
                                        }
                                        selected.set(new_selected);
                                        let mut new_chosen: Vec<Option<String>> = vec![None; lib.len()];
                                        for (id, v) in &prof.chosen {
                                            if let Some(&i) = id_to_idx.get(id) { new_chosen[i] = Some(v.clone()); }
                                        }
                                        chosen.set(new_chosen);
                                        let mut new_alts: Vec<Vec<String>> = vec![Vec::new(); lib.len()];
                                        for (id, alts) in &prof.custom_alternatives {
                                            if let Some(&i) = id_to_idx.get(id) { new_alts[i] = alts.clone(); }
                                        }
                                        custom_alts.set(new_alts);
                                        custom_rules.set(prof.custom_rules.clone());
                                        custom_domains.set(prof.custom_domains.clone());
                                        // Restore selected_domains. Legacy profiles
                                        // without this field fall back to the curated
                                        // default set + any custom domains in the
                                        // profile so canonical rules still gate correctly.
                                        let mut dom_set: HashSet<String> = prof.selected_domains.iter().cloned().collect();
                                        if dom_set.is_empty() {
                                            for d in DEFAULT_SELECTED_DOMAINS { dom_set.insert(d.to_string()); }
                                        }
                                        for cd in &prof.custom_domains { dom_set.insert(cd.clone()); }
                                        selected_domains.set(dom_set);
                                        out_dir.set(prof.out_dir.clone());
                                        repos.set(prof.repos.clone());
                                        let mut dr: HashMap<String, HashSet<String>> = HashMap::new();
                                        for (d, rs) in &prof.domain_repos {
                                            dr.insert(d.clone(), rs.iter().cloned().collect());
                                        }
                                        domain_repos.set(dr);
                                        missing_ids_warning.set(missing);
                                    }
                                    pending_recovery.set(None);
                                },
                                "Resume"
                            }
                        }
                    }
                }
            }

            // Delete-confirm banner: shows when the user clicks the × on a
            // custom rule or a custom domain. Confirm performs the deletion
            // (cascading rule deletes for domains); Cancel dismisses.
            if let Some(target) = pending_delete.read().clone() {
                {
                    let (header, body): (String, String) = match &target {
                        DeleteTarget::CustomRule(idx) => {
                            let rules = custom_rules.read();
                            let name = rules
                                .get(*idx)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "(unnamed)".to_string());
                            (
                                format!("Delete custom rule \"{name}\"?"),
                                "This cannot be undone.".to_string(),
                            )
                        }
                        DeleteTarget::CustomDomain(name) => {
                            let count = custom_rules
                                .read()
                                .iter()
                                .filter(|c| &c.domain == name)
                                .count();
                            let body = if count == 0 {
                                "This domain has no custom rules. This cannot be undone.".to_string()
                            } else if count == 1 {
                                "Deleting this domain will also delete 1 custom rule scoped to it. This cannot be undone.".to_string()
                            } else {
                                format!("Deleting this domain will also delete {count} custom rules scoped to it. This cannot be undone.")
                            };
                            (format!("Delete custom domain \"{name}\"?"), body)
                        }
                    };
                    rsx! {
                        div { style: "background:#ffe6e6; border:1px solid #d99; border-radius:6px; padding:10px 12px; margin-bottom:8px;",
                            div { style: "font-weight:600; margin-bottom:6px;", "{header}" }
                            div { style: "color:#444; font-size:0.9em; margin-bottom:8px;", "{body}" }
                            div { style: "display:flex; gap:8px;",
                                button {
                                    style: "background:#a00; color:white; border:none; padding:4px 10px; border-radius:4px; cursor:pointer;",
                                    onclick: {
                                        let target = target.clone();
                                        move |_| {
                                            match &target {
                                                DeleteTarget::CustomRule(idx) => {
                                                    let i = *idx;
                                                    custom_rules.with_mut(|v| {
                                                        if i < v.len() {
                                                            v.remove(i);
                                                        }
                                                    });
                                                    // If the edit form is open, close it (the
                                                    // index is now stale: either it pointed at
                                                    // the deleted rule, or at one whose index
                                                    // has shifted down by one). Simpler to drop
                                                    // the in-flight edit than to try to
                                                    // re-target it.
                                                    if editing_custom_idx.peek().is_some() {
                                                        editing_custom_idx.set(None);
                                                        custom_name.set(String::new());
                                                        custom_body.set(String::new());
                                                    }
                                                }
                                                DeleteTarget::CustomDomain(name) => {
                                                    let n = name.clone();
                                                    custom_rules.with_mut(|v| {
                                                        v.retain(|c| c.domain != n);
                                                    });
                                                    custom_domains.with_mut(|v| {
                                                        v.retain(|d| d != &n);
                                                    });
                                                    selected_domains.with_mut(|s| {
                                                        s.remove(&n);
                                                    });
                                                    expanded.with_mut(|s| {
                                                        s.remove(&n);
                                                    });
                                                    // Same defensive cleanup as the rule
                                                    // delete path: any in-flight edit may
                                                    // have pointed at a rule that just got
                                                    // dropped with the domain.
                                                    if editing_custom_idx.peek().is_some() {
                                                        editing_custom_idx.set(None);
                                                        custom_name.set(String::new());
                                                        custom_body.set(String::new());
                                                    }
                                                }
                                            }
                                            pending_delete.set(None);
                                        }
                                    },
                                    "Delete"
                                }
                                button {
                                    onclick: move |_| pending_delete.set(None),
                                    "Cancel"
                                }
                            }
                        }
                    }
                }
            }

            // Exit-confirm banner: shows when the user clicks the Exit
            // button OR clicks the OS window close button. Save-and-exit
            // opens the save dialog, then clears autosave and closes the
            // window. Discard-and-exit clears autosave and closes. Cancel
            // dismisses the prompt and keeps the session running.
            if exit_prompt_open() {
                div { style: "background:#fff0e5; border:1px solid #d99b6a; border-radius:6px; padding:10px 12px; margin-bottom:8px;",
                    div { style: "font-weight:600; margin-bottom:6px;",
                        "Save your work before closing?"
                    }
                    div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                        "Closing the app will clear the in-progress autosave. Save a profile now to keep your selection, or discard and exit."
                    }
                    div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                        button {
                            onclick: {
                                let window_ctx = window_ctx.clone();
                                move |_| {
                                    let saved = if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name("camerata.profile.json")
                                        .save_file()
                                    {
                                        let snap = snapshot_profile(
                                            &principles.read(),
                                            &selected.read(),
            &selected_domains.read(),
                                            &chosen.read(),
                                            &custom_alts.read(),
                                            &custom_rules.read(),
                                            &custom_domains.read(),
                                            &out_dir.read(),
                                            &repos.read(),
                                            &domain_repos.read(),
                                        );
                                        match snap.save(&path) {
                                            Ok(_) => Some(path),
                                            Err(e) => {
                                                status.set(format!("Error saving profile: {e}"));
                                                return;
                                            }
                                        }
                                    } else {
                                        // User cancelled the save dialog; keep
                                        // the prompt open so they can pick
                                        // again or choose Discard.
                                        return;
                                    };
                                    if saved.is_some() {
                                        clear_autosave();
                                        exit_prompt_open.set(false);
                                        window_ctx.close();
                                    }
                                }
                            },
                            "Save and exit"
                        }
                        button {
                            onclick: {
                                let window_ctx = window_ctx.clone();
                                move |_| {
                                    clear_autosave();
                                    exit_prompt_open.set(false);
                                    window_ctx.close();
                                }
                            },
                            "Discard and exit"
                        }
                        button {
                            onclick: move |_| exit_prompt_open.set(false),
                            "Cancel"
                        }
                    }
                }
            }

            // Generate-confirm banner: clicking Generate first opens this
            // prompt so the user can durably persist the current selection
            // before the scaffold runs. The autosave temp file is cleared
            // on successful generate from either path.
            if pending_generate() {
                div { style: "background:#e6f3ff; border:1px solid #6aa1e2; border-radius:6px; padding:10px 12px; margin-bottom:8px;",
                    div { style: "font-weight:600; margin-bottom:6px;",
                        "Save your profile before generating?"
                    }
                    div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                        "Generating will write the scaffolded files into the configured output. The in-progress autosave will be cleared either way."
                    }
                    div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                        button {
                            onclick: move |_| {
                                // Save dialog first; if the user cancels, do
                                // NOT proceed to generate (Cancel here means
                                // they want a save and didn't get one).
                                let saved = if let Some(path) = rfd::FileDialog::new()
                                    .set_file_name("camerata.profile.json")
                                    .save_file()
                                {
                                    let snap = snapshot_profile(
                                        &principles.read(),
                                        &selected.read(),
            &selected_domains.read(),
                                        &chosen.read(),
                                        &custom_alts.read(),
                                        &custom_rules.read(),
                                        &custom_domains.read(),
                                        &out_dir.read(),
                                        &repos.read(),
                                        &domain_repos.read(),
                                    );
                                    match snap.save(&path) {
                                        Ok(_) => Some(path),
                                        Err(e) => {
                                            status.set(format!("Error saving profile: {e}"));
                                            return;
                                        }
                                    }
                                } else {
                                    None
                                };
                                if saved.is_none() {
                                    // User cancelled the save dialog; treat
                                    // as "go back," keep banner open so
                                    // they can choose again.
                                    return;
                                }
                                let out = run_generate(
                                    &principles.read(),
                                    &selected.read(),
                                    &chosen.read(),
                                    &custom_rules.read(),
                                    &out_dir.read(),
                                    &repos.read(),
                                    &domain_repos.read(),
                                );
                                match out {
                                    Ok(msg) => {
                                        status.set(format!(
                                            "Profile saved to {}. {}",
                                            saved.unwrap().display(),
                                            msg
                                        ));
                                        clear_autosave();
                                    }
                                    Err(e) => status.set(e),
                                }
                                pending_generate.set(false);
                            },
                            "Save profile and generate"
                        }
                        button {
                            onclick: move |_| {
                                let out = run_generate(
                                    &principles.read(),
                                    &selected.read(),
                                    &chosen.read(),
                                    &custom_rules.read(),
                                    &out_dir.read(),
                                    &repos.read(),
                                    &domain_repos.read(),
                                );
                                match out {
                                    Ok(msg) => { status.set(msg); clear_autosave(); }
                                    Err(e) => status.set(e),
                                }
                                pending_generate.set(false);
                            },
                            "Generate without saving"
                        }
                        button {
                            onclick: move |_| pending_generate.set(false),
                            "Cancel"
                        }
                    }
                }
            }

            // Missing-ids warning: shows after Load Profile or Resume if the
            // profile references canonical-rule ids that no longer exist in
            // the current library (renamed or removed).
            if !missing_ids_warning.read().is_empty() {
                div { style: "background:#ffe6e6; border:1px solid #d99; border-radius:6px; padding:8px 10px; margin-bottom:8px;",
                    div { style: "font-weight:600; margin-bottom:4px;",
                        "Some rules from this profile are no longer in the library and were skipped:"
                    }
                    ul { style: "margin:4px 0 4px 18px; padding:0;",
                        for id in missing_ids_warning.read().iter() {
                            li { key: "{id}", style: "font-family:monospace; font-size:0.9em;", "{id}" }
                        }
                    }
                    button {
                        style: "margin-top:4px;",
                        onclick: move |_| missing_ids_warning.set(Vec::new()),
                        "Dismiss"
                    }
                }
            }

            // Action bar.
            div { style: "display:flex; gap:8px; align-items:center; margin-bottom:8px;",
                span { "Output:" }
                input {
                    r#type: "text",
                    value: "{out_dir}",
                    style: "flex:1; padding:5px;",
                    oninput: move |e| out_dir.set(e.value()),
                }
                button {
                    onclick: move |_| {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            out_dir.set(p.display().to_string());
                        }
                    },
                    "Pick…"
                }
                button {
                    onclick: move |_| {
                        // Open the Generate-confirm banner. The actual emit
                        // happens after the user picks Save-and-generate or
                        // Generate-without-saving (the autosave temp file is
                        // cleared either way on success).
                        pending_generate.set(true);
                        status.set(String::new());
                    },
                    "Generate"
                }
                button {
                    onclick: move |_| {
                        let snap = snapshot_profile(
                            &principles.read(),
                            &selected.read(),
            &selected_domains.read(),
                            &chosen.read(),
                            &custom_alts.read(),
                            &custom_rules.read(),
                            &custom_domains.read(),
                            &out_dir.read(),
                            &repos.read(),
                            &domain_repos.read(),
                        );
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("camerata.profile.json")
                            .save_file()
                        {
                            match snap.save(&path) {
                                Ok(_) => status.set(format!("Saved profile to {}", path.display())),
                                Err(e) => status.set(format!("Error: {e}")),
                            }
                        }
                    },
                    "Save Profile…"
                }
                button {
                    onclick: move |_| {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Camerata profile", &["json"])
                            .pick_file()
                        {
                            match Profile::load(&path) {
                                Ok(prof) => {
                                    let lib = principles.read().clone();
                                    let lib_ids: Vec<String> = lib.iter().map(|p| p.id.clone()).collect();
                                    let missing = prof.missing_ids(lib_ids.iter().map(|s| s.as_str()));
                                    let id_to_idx: HashMap<String, usize> = lib.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
                                    let mut new_selected = vec![false; lib.len()];
                                    for id in &prof.selected_ids {
                                        if let Some(&i) = id_to_idx.get(id) { new_selected[i] = true; }
                                    }
                                    selected.set(new_selected);
                                    let mut new_chosen: Vec<Option<String>> = vec![None; lib.len()];
                                    for (id, v) in &prof.chosen {
                                        if let Some(&i) = id_to_idx.get(id) { new_chosen[i] = Some(v.clone()); }
                                    }
                                    chosen.set(new_chosen);
                                    let mut new_alts: Vec<Vec<String>> = vec![Vec::new(); lib.len()];
                                    for (id, alts) in &prof.custom_alternatives {
                                        if let Some(&i) = id_to_idx.get(id) { new_alts[i] = alts.clone(); }
                                    }
                                    custom_alts.set(new_alts);
                                    custom_rules.set(prof.custom_rules.clone());
                                    custom_domains.set(prof.custom_domains.clone());
                                    out_dir.set(prof.out_dir.clone());
                                    repos.set(prof.repos.clone());
                                    let mut dr: HashMap<String, HashSet<String>> = HashMap::new();
                                    for (d, rs) in &prof.domain_repos {
                                        dr.insert(d.clone(), rs.iter().cloned().collect());
                                    }
                                    domain_repos.set(dr);
                                    missing_ids_warning.set(missing);
                                    status.set(format!("Loaded profile from {}", path.display()));
                                }
                                Err(e) => status.set(format!("Error loading profile: {e}")),
                            }
                        }
                    },
                    "Load Profile…"
                }
                button {
                    onclick: move |_| {
                        let plist = principles.read();
                        let sel = selected.read();
                        let cho = chosen.read();
                        let sels: Vec<Selection> = plist
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| sel[*i])
                            .map(|(i, p)| Selection { principle: p, chosen: cho[i].clone() })
                            .collect();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("camerata.selections.json")
                            .save_file()
                        {
                            match emit::selections_json(&sels) {
                                Ok(j) => match std::fs::write(&path, j) {
                                    Ok(_) => status.set(format!("Exported JSON (full content) to {}", path.display())),
                                    Err(e) => status.set(format!("Error: {e}")),
                                },
                                Err(e) => status.set(format!("Error: {e}")),
                            }
                        }
                    },
                    "Export JSON…"
                }
                button {
                    onclick: move |_| {
                        let cur = show_targets();
                        show_targets.set(!cur);
                    },
                    "Targets…"
                }
                button {
                    onclick: move |_| exit_prompt_open.set(true),
                    "Exit"
                }
            }
            if !status.read().is_empty() {
                div { style: "color:#0a6; margin-bottom:8px;", "{status}" }
            }

            if show_targets() {
                div { style: "border:1px solid #ddd; border-radius:6px; padding:10px; margin-bottom:10px; background:#fafafa;",
                    div { style: "font-weight:600; margin-bottom:6px;", "Target repos" }
                    div { style: "display:flex; gap:6px; margin-bottom:6px;",
                        input {
                            r#type: "text",
                            placeholder: "/path/to/repo",
                            value: "{new_repo}",
                            style: "flex:1; padding:4px;",
                            oninput: move |e| new_repo.set(e.value()),
                        }
                        button {
                            onclick: move |_| {
                                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                    new_repo.set(p.display().to_string());
                                }
                            },
                            "Pick…"
                        }
                        button {
                            onclick: move |_| {
                                let r = new_repo.read().trim().to_string();
                                if !r.is_empty() {
                                    repos.with_mut(|v| if !v.contains(&r) { v.push(r.clone()); });
                                    new_repo.set(String::new());
                                }
                            },
                            "Add repo"
                        }
                        button {
                            onclick: move |_| {
                                let r = new_repo.read().trim().to_string();
                                if r.is_empty() {
                                    return;
                                }
                                match std::fs::create_dir_all(&r) {
                                    Ok(_) => {
                                        let _ = std::process::Command::new("git")
                                            .arg("init")
                                            .current_dir(&r)
                                            .output();
                                        repos.with_mut(|v| if !v.contains(&r) { v.push(r.clone()); });
                                        new_repo.set(String::new());
                                        status.set(format!("Created repo {r}"));
                                    }
                                    Err(e) => status.set(format!("Error creating {r}: {e}")),
                                }
                            },
                            "Create new repo"
                        }
                    }
                    div { style: "margin-bottom:8px;",
                        for (ri , r) in repos.read().iter().enumerate() {
                            {
                                let idx = ri;
                                let label = r.clone();
                                rsx! {
                                    span {
                                        key: "{ri}",
                                        style: "display:inline-flex; align-items:center; gap:4px; background:#e7e7e7; border-radius:10px; padding:2px 8px; margin:2px; font-size:0.85em;",
                                        "{label}"
                                        button {
                                            style: "border:none; background:none; cursor:pointer; color:#a00;",
                                            onclick: move |_| {
                                                repos.with_mut(|v| {
                                                    if idx < v.len() {
                                                        v.remove(idx);
                                                    }
                                                });
                                            },
                                            "×"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if repos.read().is_empty() {
                        div { style: "color:#888; font-size:0.85em;",
                            "Add or create a repo, then map domains to it. Unmapped domains use the default output above."
                        }
                    } else {
                        div { style: "font-size:0.85em; color:#777; margin-bottom:4px;",
                            "Map each domain to one or more repos (unmapped → default output):"
                        }
                        for (domain , _open , _rows) in groups.iter().filter(|(d, _, rows)| {
                            // Hide meta-doc domains entirely — they never emit.
                            if is_meta_domain(d.as_str()) { return false; }
                            // For canonical (non-custom) domains, only show if at
                            // least one rule in the group is currently selected.
                            // For custom domains, only show if there is at least
                            // one custom rule scoped to it.
                            let is_custom = custom_domains.read().contains(d);
                            if is_custom {
                                custom_rules.read().iter().any(|c| &c.domain == d)
                            } else {
                                let sel = selected.read();
                                rows.iter().any(|(i, _, _)| sel.get(*i).copied().unwrap_or(false))
                            }
                        }) {
                            div { style: "display:flex; align-items:center; gap:10px; padding:2px 0; flex-wrap:wrap;",
                                span { style: "min-width:190px;", "{domain_label(domain)}" }
                                for r in repos.read().iter() {
                                    {
                                        let dom = domain.clone();
                                        let repo = r.clone();
                                        let checked = domain_repos
                                            .read()
                                            .get(domain)
                                            .map(|s| s.contains(r))
                                            .unwrap_or(false);
                                        rsx! {
                                            label { style: "display:inline-flex; align-items:center; gap:3px; font-size:0.85em;",
                                                input {
                                                    r#type: "checkbox",
                                                    checked,
                                                    onchange: move |_| {
                                                        let dom = dom.clone();
                                                        let repo = repo.clone();
                                                        domain_repos.with_mut(|m| {
                                                            let set = m.entry(dom).or_default();
                                                            if !set.remove(&repo) {
                                                                set.insert(repo);
                                                            }
                                                        });
                                                    },
                                                }
                                                "{r}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Two columns.
            div { style: "display:flex; gap:14px; align-items:flex-start;",

                // Left: selection list with collapsible domains.
                div { style: "width:46%; border-right:1px solid #ddd; padding-right:10px; max-height:74vh; overflow:auto;",
                    div { style: "display:flex; margin-bottom:6px; color:#777; font-size:0.9em;",
                        span { style: "margin-left:auto;", "{selected_count} selected" }
                    }

                    for (domain , open , rows) in groups.iter() {
                        div { style: "margin-bottom:4px;",
                            div { style: "display:flex; align-items:center; gap:6px; background:#f3f3f3; border:1px solid #ddd; border-radius:4px; padding:6px;",
                                // Domain-level checkbox. Hidden for meta-doc
                                // domains (howto, contributing) and custom
                                // domains (which the user creates explicitly
                                // and are always active). Toggling cascades
                                // to per-rule selections.
                                if !is_meta_domain(domain.as_str())
                                    && !custom_domains.read().contains(domain)
                                {
                                    input {
                                        r#type: "checkbox",
                                        checked: selected_domains.read().contains(domain),
                                        onchange: {
                                            let d = domain.clone();
                                            let row_indices: Vec<usize> = rows.iter().map(|(i, _, _)| *i).collect();
                                            let defaults_for_domain: Vec<bool> = rows.iter().map(|(i, _, _)| plist[*i].default).collect();
                                            move |_| {
                                                let now_selected = !selected_domains.read().contains(&d);
                                                if now_selected {
                                                    selected_domains.with_mut(|s| { s.insert(d.clone()); });
                                                    // Cascade: set default rules to checked, others to unchecked
                                                    selected.with_mut(|v| {
                                                        for (k, idx) in row_indices.iter().enumerate() {
                                                            v[*idx] = defaults_for_domain[k];
                                                        }
                                                    });
                                                } else {
                                                    selected_domains.with_mut(|s| { s.remove(&d); });
                                                    // Cascade: unselect every rule in this domain
                                                    selected.with_mut(|v| {
                                                        for idx in &row_indices {
                                                            v[*idx] = false;
                                                        }
                                                    });
                                                }
                                            }
                                        },
                                    }
                                }
                                button {
                                    style: "flex:1; text-align:left; background:none; border:none; cursor:pointer; font-weight:600; padding:0;",
                                    onclick: {
                                        let d = domain.clone();
                                        move |_| {
                                            let d = d.clone();
                                            expanded.with_mut(|s| {
                                                if !s.remove(&d) {
                                                    s.insert(d);
                                                }
                                            });
                                        }
                                    },
                                    if *open { "▾  " } else { "▸  " }
                                    "{domain_label(domain)}"
                                }
                                // Edit + delete buttons — only on user-created
                                // custom domains. Edit opens a rename form in
                                // the right pane; delete removes the domain
                                // plus every custom rule scoped to it (after
                                // a confirmation banner so accidents are caught).
                                if custom_domains.read().contains(domain) {
                                    button {
                                        style: "background:none; border:none; cursor:pointer; color:#1452a3; padding:0 4px;",
                                        title: "Rename this custom domain",
                                        onclick: {
                                            let d = domain.clone();
                                            move |_| {
                                                if (editing_custom_idx.peek().is_some()
                                                    || adding_custom.peek().is_some()
                                                    || *adding_domain.peek()
                                                    || editing_custom_domain.peek().is_some())
                                                    && *custom_form_dirty.peek()
                                                {
                                                    pending_nav.set(Some(PendingNav::RenameCustomDomain(d.clone())));
                                                    return;
                                                }
                                                editing_custom_domain.set(Some(d.clone()));
                                                renamed_domain_name.set(d.clone());
                                                // Close any other right-pane mode.
                                                adding_custom.set(None);
                                                adding_domain.set(false);
                                                editing_custom_idx.set(None);
                                                detail.set(None);
                                            }
                                        },
                                        "✎"
                                    }
                                    button {
                                        style: "background:none; border:none; cursor:pointer; color:#a00; font-weight:600; padding:0 4px;",
                                        title: "Delete this custom domain and all its custom rules",
                                        onclick: {
                                            let d = domain.clone();
                                            move |_| {
                                                pending_delete.set(Some(DeleteTarget::CustomDomain(d.clone())));
                                            }
                                        },
                                        "×"
                                    }
                                }
                            }
                            if *open {
                                // Defaults / All / Clear toggle per-rule
                                // selection, which is meaningless in the
                                // meta-doc domains (howto, contributing) and
                                // in custom domains (they only hold custom
                                // rules; no canonical checkboxes to toggle).
                                if !is_meta_domain(domain.as_str())
                                    && !custom_domains.read().contains(domain)
                                {
                                    {
                                        let defaults: Vec<(usize, bool)> = rows
                                            .iter()
                                            .map(|(i, _, _)| (*i, plist[*i].default))
                                            .collect();
                                        let idxs_all: Vec<usize> = rows.iter().map(|(i, _, _)| *i).collect();
                                        let idxs_clear = idxs_all.clone();
                                        let btn = "padding:2px 8px; font-size:0.8em; cursor:pointer;";
                                        rsx! {
                                            div { style: "display:flex; gap:4px; padding:2px 0 4px 10px;",
                                                button {
                                                    style: "{btn}",
                                                    onclick: move |_| selected.with_mut(|s| { for (i, d) in &defaults { s[*i] = *d; } }),
                                                    "Defaults"
                                                }
                                                button {
                                                    style: "{btn}",
                                                    onclick: move |_| selected.with_mut(|s| { for i in &idxs_all { s[*i] = true; } }),
                                                    "All"
                                                }
                                                button {
                                                    style: "{btn}",
                                                    onclick: move |_| selected.with_mut(|s| { for i in &idxs_clear { s[*i] = false; } }),
                                                    "Clear"
                                                }
                                            }
                                        }
                                    }
                                }
                                for (idx , title , tag) in rows.iter() {
                                    {
                                        // Meta-doc domains (howto, contributing) render their
                                        // rules at full opacity even though they have no
                                        // selection checkbox: the rules ARE the documentation
                                        // and dimming them implies "disabled," which they are
                                        // not. Selectable domains keep the gray-out behavior
                                        // when not currently selected.
                                        let domain_active = is_meta_domain(domain.as_str())
                                            || selected_domains.read().contains(domain)
                                            || custom_domains.read().contains(domain);
                                        let row_style = if domain_active {
                                            "display:flex; align-items:center; gap:6px; padding:2px 0 2px 10px;"
                                        } else {
                                            "display:flex; align-items:center; gap:6px; padding:2px 0 2px 10px; opacity:0.5;"
                                        };
                                        rsx! {
                                            div {
                                                key: "{idx}",
                                                style: "{row_style}",
                                                if !is_meta_domain(domain.as_str()) {
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected.read()[*idx],
                                                        disabled: !domain_active,
                                                        onchange: {
                                                            let i = *idx;
                                                            move |_| {
                                                                if domain_active {
                                                                    selected.with_mut(|s| s[i] = !s[i]);
                                                                }
                                                            }
                                                        },
                                                    }
                                                }
                                                button {
                                                    style: "background:none; border:none; text-align:left; cursor:pointer; color:#1452a3; padding:0;",
                                                    onclick: {
                                                        let i = *idx;
                                                        move |_| {
                                                            if (editing_custom_idx.peek().is_some()
                                                    || adding_custom.peek().is_some()
                                                    || *adding_domain.peek()
                                                    || editing_custom_domain.peek().is_some())
                                                                && *custom_form_dirty.peek()
                                                            {
                                                                pending_nav.set(Some(PendingNav::OpenCanonical(i)));
                                                                return;
                                                            }
                                                            editing_custom_idx.set(None);
                                                            editing_custom_domain.set(None);
                                                            adding_custom.set(None);
                                                            adding_domain.set(false);
                                                            detail.set(Some(i));
                                                        }
                                                    },
                                                    "{tag} {title}"
                                                }
                                            }
                                        }
                                    }
                                }
                                for (k , c) in custom_rules.read().iter().enumerate() {
                                    if &c.domain == domain {
                                        div {
                                            key: "c{k}",
                                            style: "display:flex; align-items:center; gap:6px; padding:2px 0 2px 10px; color:#555; font-size:0.9em;",
                                            button {
                                                style: "background:none; border:none; cursor:pointer; color:#555; padding:0; text-align:left; flex:1; font-size:1em;",
                                                title: "Open to view or edit",
                                                onclick: move |_| {
                                                    // If a different custom rule form is
                                                    // open with unsaved edits, defer; the
                                                    // save-or-discard prompt will pick the
                                                    // navigation back up after the user
                                                    // resolves it.
                                                    if (editing_custom_idx.peek().is_some()
                                                    || adding_custom.peek().is_some()
                                                    || *adding_domain.peek()
                                                    || editing_custom_domain.peek().is_some())
                                                        && *custom_form_dirty.peek()
                                                    {
                                                        pending_nav.set(Some(PendingNav::OpenCustomRule(k)));
                                                        return;
                                                    }
                                                    let snapshot = custom_rules.read().get(k).cloned();
                                                    if let Some(rule) = snapshot {
                                                        custom_name.set(rule.name);
                                                        custom_body.set(rule.body);
                                                        custom_form_dirty.set(false);
                                                        editing_custom_idx.set(Some(k));
                                                        // Close other right-pane modes.
                                                        adding_custom.set(None);
                                                        adding_domain.set(false);
                                                        editing_custom_domain.set(None);
                                                        detail.set(None);
                                                    }
                                                },
                                                "✎ {c.name}"
                                            }
                                            button {
                                                style: "background:none; border:none; cursor:pointer; color:#a00; padding:0 4px;",
                                                title: "Delete this custom rule",
                                                onclick: move |_| {
                                                    pending_delete.set(Some(DeleteTarget::CustomRule(k)));
                                                },
                                                "×"
                                            }
                                        }
                                    }
                                }
                                if !is_meta_domain(domain.as_str()) {
                                    button {
                                        style: "margin:4px 0 6px 10px; background:none; border:1px dashed #aaa; border-radius:4px; padding:3px 8px; cursor:pointer; color:#1452a3;",
                                        onclick: {
                                            let d = domain.clone();
                                            move |_| {
                                                if (editing_custom_idx.peek().is_some()
                                                    || adding_custom.peek().is_some()
                                                    || *adding_domain.peek()
                                                    || editing_custom_domain.peek().is_some())
                                                    && *custom_form_dirty.peek()
                                                {
                                                    pending_nav.set(Some(PendingNav::AddCustomRule(d.clone())));
                                                    return;
                                                }
                                                adding_custom.set(Some(d.clone()));
                                                detail.set(None);
                                                editing_custom_idx.set(None);
                                                editing_custom_domain.set(None);
                                                custom_name.set(String::new());
                                                custom_body.set(String::new());
                                                custom_form_dirty.set(false);
                                            }
                                        },
                                        "+ custom rule"
                                    }
                                }
                            }
                        }
                    }
                    // Bottom of left sidebar: button to create a new custom
                    // domain (a domain holding only user-authored rules, no
                    // canonical principles).
                    button {
                        style: "margin-top:10px; background:none; border:1px dashed #aaa; border-radius:4px; padding:5px 10px; cursor:pointer; color:#1452a3; width:100%; box-sizing:border-box;",
                        onclick: move |_| {
                            if (editing_custom_idx.peek().is_some()
                                                    || adding_custom.peek().is_some()
                                                    || *adding_domain.peek()
                                                    || editing_custom_domain.peek().is_some())
                                && *custom_form_dirty.peek()
                            {
                                pending_nav.set(Some(PendingNav::AddCustomDomain));
                                return;
                            }
                            adding_domain.set(true);
                            adding_custom.set(None);
                            editing_custom_idx.set(None);
                            editing_custom_domain.set(None);
                            detail.set(None);
                            new_domain_name.set(String::new());
                        },
                        "+ Custom domain"
                    }
                }

                // Right: custom-domain form (when adding) OR custom-rule form
                // OR principle detail OR empty-state tip.
                div { style: "width:54%; max-height:74vh; overflow:auto;",
                    if adding_domain() {
                        div {
                            h3 { style: "margin:0;", "Add a custom domain" }
                            div { style: "color:#666; margin:4px 0 8px 0;",
                                "A custom domain holds only your own rules. It will not contain canonical principles from the camerata library."
                            }
                            // Save-or-discard prompt for click-away with text in the field.
                            if pending_nav.read().is_some() {
                                div { style: "background:#fff0e5; border:1px solid #d99b6a; border-radius:6px; padding:8px 10px; margin-bottom:8px;",
                                    div { style: "font-weight:600; margin-bottom:6px;", "Unsaved new domain" }
                                    div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                                        "Add the new custom domain before navigating away, or discard it."
                                    }
                                    div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                                        button {
                                            onclick: move |_| {
                                                let name = new_domain_name.read().trim().to_string();
                                                if !name.is_empty() {
                                                    custom_domains.with_mut(|v| {
                                                        if !v.contains(&name) { v.push(name.clone()); }
                                                    });
                                                    selected_domains.with_mut(|s| { s.insert(name.clone()); });
                                                    expanded.with_mut(|s| { s.insert(name); });
                                                }
                                                custom_form_dirty.set(false);
                                                let nav = pending_nav.read().clone();
                                                pending_nav.set(None);
                                                apply_pending_nav(
                                                    nav, custom_name, custom_body, editing_custom_idx,
                                                    editing_custom_domain, renamed_domain_name, adding_custom,
                                                    adding_domain, new_domain_name, detail, custom_form_dirty,
                                                    &custom_rules.read(),
                                                );
                                            },
                                            "Save"
                                        }
                                        button {
                                            onclick: move |_| {
                                                custom_form_dirty.set(false);
                                                let nav = pending_nav.read().clone();
                                                pending_nav.set(None);
                                                apply_pending_nav(
                                                    nav, custom_name, custom_body, editing_custom_idx,
                                                    editing_custom_domain, renamed_domain_name, adding_custom,
                                                    adding_domain, new_domain_name, detail, custom_form_dirty,
                                                    &custom_rules.read(),
                                                );
                                            },
                                            "Discard"
                                        }
                                        button {
                                            onclick: move |_| pending_nav.set(None),
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                            input {
                                r#type: "text",
                                placeholder: "Domain name (e.g. our-internal-rules)",
                                value: "{new_domain_name}",
                                style: "width:100%; padding:5px; margin-bottom:6px; box-sizing:border-box;",
                                oninput: move |e| {
                                    new_domain_name.set(e.value());
                                    custom_form_dirty.set(true);
                                },
                            }
                            div { style: "margin-top:6px; display:flex; gap:6px;",
                                button {
                                    onclick: move |_| {
                                        let name = new_domain_name.read().trim().to_string();
                                        if !name.is_empty() {
                                            custom_domains.with_mut(|v| {
                                                if !v.contains(&name) { v.push(name.clone()); }
                                            });
                                            // Custom domains are always "active" so
                                            // their rules emit and check freely.
                                            selected_domains.with_mut(|s| { s.insert(name.clone()); });
                                            expanded.with_mut(|s| { s.insert(name); });
                                        }
                                        new_domain_name.set(String::new());
                                        custom_form_dirty.set(false);
                                        adding_domain.set(false);
                                    },
                                    "Add domain"
                                }
                                button {
                                    onclick: move |_| {
                                        new_domain_name.set(String::new());
                                        custom_form_dirty.set(false);
                                        adding_domain.set(false);
                                    },
                                    "Cancel"
                                }
                            }
                        }
                    } else if let Some(dom) = adding_custom() {
                        div {
                            h3 { style: "margin:0;", "Add a custom rule" }
                            div { style: "color:#666; margin:4px 0 8px 0;", "Domain: {domain_label(&dom)}" }
                            // Same save-or-discard prompt as the edit form.
                            // Appears when the user has typed something and
                            // tried to navigate elsewhere.
                            if pending_nav.read().is_some() {
                                div { style: "background:#fff0e5; border:1px solid #d99b6a; border-radius:6px; padding:8px 10px; margin-bottom:8px;",
                                    div { style: "font-weight:600; margin-bottom:6px;",
                                        "Unsaved new rule"
                                    }
                                    div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                                        "Add the new custom rule before navigating away, or discard it."
                                    }
                                    div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                                        button {
                                            onclick: move |_| {
                                                let name = custom_name.read().clone();
                                                let body = custom_body.read().clone();
                                                let domain = adding_custom().unwrap_or_else(|| "*".to_string());
                                                if !name.trim().is_empty() || !body.trim().is_empty() {
                                                    custom_rules.with_mut(|v| v.push(CustomRule { name, body, domain }));
                                                }
                                                custom_form_dirty.set(false);
                                                let nav = pending_nav.read().clone();
                                                pending_nav.set(None);
                                                apply_pending_nav(
                                                    nav,
                                                    custom_name,
                                                    custom_body,
                                                    editing_custom_idx,
                                                    editing_custom_domain,
                                                    renamed_domain_name,
                                                    adding_custom,
                                                    adding_domain,
                                                    new_domain_name,
                                                    detail,
                                                    custom_form_dirty,
                                                    &custom_rules.read(),
                                                );
                                            },
                                            "Save"
                                        }
                                        button {
                                            onclick: move |_| {
                                                custom_form_dirty.set(false);
                                                let nav = pending_nav.read().clone();
                                                pending_nav.set(None);
                                                apply_pending_nav(
                                                    nav,
                                                    custom_name,
                                                    custom_body,
                                                    editing_custom_idx,
                                                    editing_custom_domain,
                                                    renamed_domain_name,
                                                    adding_custom,
                                                    adding_domain,
                                                    new_domain_name,
                                                    detail,
                                                    custom_form_dirty,
                                                    &custom_rules.read(),
                                                );
                                            },
                                            "Discard"
                                        }
                                        button {
                                            onclick: move |_| pending_nav.set(None),
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                            input {
                                r#type: "text",
                                placeholder: "Rule name",
                                value: "{custom_name}",
                                style: "width:100%; padding:5px; margin-bottom:6px; box-sizing:border-box;",
                                oninput: move |e| {
                                    custom_name.set(e.value());
                                    custom_form_dirty.set(true);
                                },
                            }
                            textarea {
                                placeholder: "Rule text and the context it requires…",
                                rows: "7",
                                style: "width:100%; padding:6px; box-sizing:border-box;",
                                value: "{custom_body}",
                                oninput: move |e| {
                                    custom_body.set(e.value());
                                    custom_form_dirty.set(true);
                                },
                            }
                            div { style: "margin-top:6px; display:flex; gap:6px;",
                                button {
                                    onclick: move |_| {
                                        let name = custom_name.read().clone();
                                        let body = custom_body.read().clone();
                                        let domain = adding_custom().unwrap_or_else(|| "*".to_string());
                                        if !name.trim().is_empty() || !body.trim().is_empty() {
                                            custom_rules.with_mut(|v| v.push(CustomRule { name, body, domain }));
                                        }
                                        custom_name.set(String::new());
                                        custom_body.set(String::new());
                                        custom_form_dirty.set(false);
                                        adding_custom.set(None);
                                    },
                                    "Add rule"
                                }
                                button {
                                    onclick: move |_| {
                                        custom_name.set(String::new());
                                        custom_body.set(String::new());
                                        custom_form_dirty.set(false);
                                        adding_custom.set(None);
                                    },
                                    "Cancel"
                                }
                            }
                        }
                    } else if let Some(old_name) = editing_custom_domain() {
                        // Rename a custom domain. The form is a single name
                        // field; Save propagates the new name to every
                        // signal that previously referenced the old name.
                        div {
                            h3 { style: "margin:0;",
                                if *custom_form_dirty.read() { "Rename custom domain" } else { "Custom domain" }
                            }
                            div { style: "color:#666; margin:4px 0 8px 0;", "Current name: {old_name}" }
                            if pending_nav.read().is_some() {
                                {
                                let old = old_name.clone();
                                rsx! {
                                    div { style: "background:#fff0e5; border:1px solid #d99b6a; border-radius:6px; padding:8px 10px; margin-bottom:8px;",
                                        div { style: "font-weight:600; margin-bottom:6px;", "Unsaved rename" }
                                        div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                                            "Save the new name before navigating away, or discard the change."
                                        }
                                        div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                                            button {
                                                onclick: {
                                                    let old = old.clone();
                                                    move |_| {
                                                        let new_name = renamed_domain_name.read().trim().to_string();
                                                        if !new_name.is_empty() && new_name != old {
                                                            custom_domains.with_mut(|v| {
                                                                for d in v.iter_mut() {
                                                                    if *d == old { *d = new_name.clone(); }
                                                                }
                                                            });
                                                            custom_rules.with_mut(|v| {
                                                                for c in v.iter_mut() {
                                                                    if c.domain == old { c.domain = new_name.clone(); }
                                                                }
                                                            });
                                                            selected_domains.with_mut(|s| {
                                                                if s.remove(&old) { s.insert(new_name.clone()); }
                                                            });
                                                            expanded.with_mut(|s| {
                                                                if s.remove(&old) { s.insert(new_name.clone()); }
                                                            });
                                                            domain_repos.with_mut(|m| {
                                                                if let Some(repos) = m.remove(&old) {
                                                                    m.insert(new_name.clone(), repos);
                                                                }
                                                            });
                                                        }
                                                        custom_form_dirty.set(false);
                                                        let nav = pending_nav.read().clone();
                                                        pending_nav.set(None);
                                                        apply_pending_nav(
                                                            nav, custom_name, custom_body, editing_custom_idx,
                                                            editing_custom_domain, renamed_domain_name, adding_custom,
                                                            adding_domain, new_domain_name, detail, custom_form_dirty,
                                                            &custom_rules.read(),
                                                        );
                                                    }
                                                },
                                                "Save"
                                            }
                                            button {
                                                onclick: move |_| {
                                                    custom_form_dirty.set(false);
                                                    let nav = pending_nav.read().clone();
                                                    pending_nav.set(None);
                                                    apply_pending_nav(
                                                        nav, custom_name, custom_body, editing_custom_idx,
                                                        editing_custom_domain, renamed_domain_name, adding_custom,
                                                        adding_domain, new_domain_name, detail, custom_form_dirty,
                                                        &custom_rules.read(),
                                                    );
                                                },
                                                "Discard"
                                            }
                                            button {
                                                onclick: move |_| pending_nav.set(None),
                                                "Cancel"
                                            }
                                        }
                                    }
                                }
                                }
                            }
                            input {
                                r#type: "text",
                                placeholder: "New domain name",
                                value: "{renamed_domain_name}",
                                style: "width:100%; padding:5px; margin-bottom:6px; box-sizing:border-box;",
                                oninput: move |e| {
                                    renamed_domain_name.set(e.value());
                                    custom_form_dirty.set(true);
                                },
                            }
                            if *custom_form_dirty.read() {
                                div { style: "margin-top:6px; display:flex; gap:6px;",
                                    button {
                                        onclick: {
                                            let old = old_name.clone();
                                            move |_| {
                                                let new_name = renamed_domain_name.read().trim().to_string();
                                                if new_name.is_empty() || new_name == old {
                                                    editing_custom_domain.set(None);
                                                    renamed_domain_name.set(String::new());
                                                    custom_form_dirty.set(false);
                                                    return;
                                                }
                                                custom_domains.with_mut(|v| {
                                                    for d in v.iter_mut() {
                                                        if *d == old { *d = new_name.clone(); }
                                                    }
                                                });
                                                custom_rules.with_mut(|v| {
                                                    for c in v.iter_mut() {
                                                        if c.domain == old { c.domain = new_name.clone(); }
                                                    }
                                                });
                                                selected_domains.with_mut(|s| {
                                                    if s.remove(&old) { s.insert(new_name.clone()); }
                                                });
                                                expanded.with_mut(|s| {
                                                    if s.remove(&old) { s.insert(new_name.clone()); }
                                                });
                                                domain_repos.with_mut(|m| {
                                                    if let Some(repos) = m.remove(&old) {
                                                        m.insert(new_name.clone(), repos);
                                                    }
                                                });
                                                editing_custom_domain.set(None);
                                                renamed_domain_name.set(String::new());
                                                custom_form_dirty.set(false);
                                            }
                                        },
                                        "Save"
                                    }
                                    button {
                                        onclick: move |_| {
                                            editing_custom_domain.set(None);
                                            renamed_domain_name.set(String::new());
                                            custom_form_dirty.set(false);
                                        },
                                        "Cancel"
                                    }
                                }
                            }
                        }
                    } else if let Some(edit_idx) = editing_custom_idx() {
                        {
                        // Same form as Add a custom rule, except Save updates
                        // the existing entry in place and the header reads
                        // "Edit". The domain is read from the existing rule
                        // so it stays put under the same group.
                        let domain_for_label = custom_rules
                            .read()
                            .get(edit_idx)
                            .map(|c| c.domain.clone())
                            .unwrap_or_else(|| "*".to_string());
                        let label = domain_label(&domain_for_label);
                        rsx! {
                            div {
                                h3 { style: "margin:0;",
                                    if *custom_form_dirty.read() { "Edit custom rule" } else { "Custom rule" }
                                }
                                div { style: "color:#666; margin:4px 0 8px 0;", "Domain: {label}" }
                                // When the user has tried to navigate away
                                // with unsaved changes, show a save/discard
                                // prompt above the form. Cancel keeps the
                                // user in the form with edits intact.
                                if pending_nav.read().is_some() {
                                    div { style: "background:#fff0e5; border:1px solid #d99b6a; border-radius:6px; padding:8px 10px; margin-bottom:8px;",
                                        div { style: "font-weight:600; margin-bottom:6px;",
                                            "Unsaved changes"
                                        }
                                        div { style: "color:#444; font-size:0.9em; margin-bottom:8px;",
                                            "Save the edits to this custom rule before navigating away, or discard them."
                                        }
                                        div { style: "display:flex; gap:8px; flex-wrap:wrap;",
                                            button {
                                                onclick: move |_| {
                                                    let name = custom_name.read().clone();
                                                    let body = custom_body.read().clone();
                                                    custom_rules.with_mut(|v| {
                                                        if let Some(rule) = v.get_mut(edit_idx) {
                                                            rule.name = name;
                                                            rule.body = body;
                                                        }
                                                    });
                                                    custom_form_dirty.set(false);
                                                    let nav = pending_nav.read().clone();
                                                    pending_nav.set(None);
                                                    apply_pending_nav(
                                                        nav,
                                                        custom_name,
                                                        custom_body,
                                                        editing_custom_idx,
                                                        editing_custom_domain,
                                                        renamed_domain_name,
                                                        adding_custom,
                                                        adding_domain,
                                                        new_domain_name,
                                                        detail,
                                                        custom_form_dirty,
                                                        &custom_rules.read(),
                                                    );
                                                },
                                                "Save"
                                            }
                                            button {
                                                onclick: move |_| {
                                                    custom_form_dirty.set(false);
                                                    let nav = pending_nav.read().clone();
                                                    pending_nav.set(None);
                                                    apply_pending_nav(
                                                        nav,
                                                        custom_name,
                                                        custom_body,
                                                        editing_custom_idx,
                                                        editing_custom_domain,
                                                        renamed_domain_name,
                                                        adding_custom,
                                                        adding_domain,
                                                        new_domain_name,
                                                        detail,
                                                        custom_form_dirty,
                                                        &custom_rules.read(),
                                                    );
                                                },
                                                "Discard"
                                            }
                                            button {
                                                onclick: move |_| pending_nav.set(None),
                                                "Cancel"
                                            }
                                        }
                                    }
                                }
                                input {
                                    r#type: "text",
                                    placeholder: "Rule name",
                                    value: "{custom_name}",
                                    style: "width:100%; padding:5px; margin-bottom:6px; box-sizing:border-box;",
                                    oninput: move |e| {
                                        custom_name.set(e.value());
                                        custom_form_dirty.set(true);
                                    },
                                }
                                textarea {
                                    placeholder: "Rule text and the context it requires…",
                                    rows: "7",
                                    style: "width:100%; padding:6px; box-sizing:border-box;",
                                    value: "{custom_body}",
                                    oninput: move |e| {
                                        custom_body.set(e.value());
                                        custom_form_dirty.set(true);
                                    },
                                }
                                if *custom_form_dirty.read() {
                                    div { style: "margin-top:6px; display:flex; gap:6px;",
                                        button {
                                            onclick: move |_| {
                                                let name = custom_name.read().clone();
                                                let body = custom_body.read().clone();
                                                custom_rules.with_mut(|v| {
                                                    if let Some(rule) = v.get_mut(edit_idx) {
                                                        rule.name = name;
                                                        rule.body = body;
                                                    }
                                                });
                                                custom_name.set(String::new());
                                                custom_body.set(String::new());
                                                custom_form_dirty.set(false);
                                                editing_custom_idx.set(None);
                                            },
                                            "Save"
                                        }
                                        button {
                                            onclick: move |_| {
                                                custom_name.set(String::new());
                                                custom_body.set(String::new());
                                                custom_form_dirty.set(false);
                                                editing_custom_idx.set(None);
                                            },
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                        }
                        }
                    } else if let Some(i) = detail_idx {
                        div {
                            h3 { style: "margin:0;", "{plist[i].title}" }
                            div { style: "color:#999; font-family:monospace; font-size:0.85em; margin-bottom:6px;", "{plist[i].id}" }
                            for (k , block) in parse_markdown_lite(&plist[i].summary).into_iter().enumerate() {
                                if let Block::H1(t) = &block {
                                    h3 { key: "b-{k}", style: "margin:14px 0 4px 0;", "{t}" }
                                }
                                if let Block::H2(t) = &block {
                                    h4 { key: "b-{k}", style: "margin:10px 0 4px 0;", "{t}" }
                                }
                                if let Block::Para(t) = &block {
                                    p { key: "b-{k}", "{t}" }
                                }
                                if let Block::List(items) = &block {
                                    ul { key: "b-{k}", style: "margin:6px 0; padding-left:20px;",
                                        for (j , it) in items.iter().enumerate() {
                                            li { key: "{j}", "{it}" }
                                        }
                                    }
                                }
                            }
                            if let Some(w) = plist[i].why.clone() {
                                p { b { "Why: " } "{w}" }
                            }
                            if !is_meta_domain(plist[i].domain.as_str()) {
                            div { style: "margin-top:8px; font-weight:600;", "Choose how to adopt this:" }
                            {
                                let is_def = chosen.read()[i].is_none();
                                rsx! {
                                    button {
                                        style: opt_style(is_def),
                                        onclick: move |_| chosen.with_mut(|c| c[i] = None),
                                        "Adopt as written (default)"
                                    }
                                }
                            }
                            for (k , alt) in plist[i].alternatives.iter().enumerate() {
                                {
                                    let altc = alt.clone();
                                    let label = alt.clone();
                                    let is_sel = chosen.read()[i].as_deref() == Some(alt.as_str());
                                    rsx! {
                                        button {
                                            key: "{k}",
                                            style: opt_style(is_sel),
                                            onclick: move |_| {
                                                let a = altc.clone();
                                                chosen.with_mut(|c| c[i] = Some(a.clone()));
                                            },
                                            "Alternative: {label}"
                                        }
                                    }
                                }
                            }
                            // User-authored alternatives for this built-in rule.
                            for (k , alt) in custom_alts.read()[i].clone().iter().enumerate() {
                                {
                                    let altc = alt.clone();
                                    let label = alt.clone();
                                    let is_sel = chosen.read()[i].as_deref() == Some(alt.as_str());
                                    rsx! {
                                        button {
                                            key: "custom-{k}",
                                            style: opt_style(is_sel),
                                            onclick: move |_| {
                                                let a = altc.clone();
                                                chosen.with_mut(|c| c[i] = Some(a.clone()));
                                            },
                                            "Your alternative: {label}"
                                        }
                                    }
                                }
                            }
                            div { style: "margin-top:10px;",
                                div { style: "font-weight:600; margin-bottom:3px;",
                                    "Add your own alternative (include the context it requires):"
                                }
                                textarea {
                                    placeholder: "Describe your alternative and the context/rationale it needs…",
                                    rows: "5",
                                    style: "width:100%; padding:6px; box-sizing:border-box;",
                                    value: "{new_alt}",
                                    oninput: move |e| new_alt.set(e.value()),
                                }
                                button {
                                    style: "margin-top:4px;",
                                    onclick: move |_| {
                                        let a = new_alt.read().clone();
                                        if !a.trim().is_empty() {
                                            custom_alts.with_mut(|v| v[i].push(a.clone()));
                                            chosen.with_mut(|c| c[i] = Some(a.clone()));
                                            new_alt.set(String::new());
                                        }
                                    },
                                    "Add alternative"
                                }
                            }
                            }
                        }
                    } else {
                        div { style: "color:#888; padding:20px 0;",
                            "Select a principle on the left, or use the + custom rule button under a domain to add your own."
                        }
                    }
                }
            }
        }
    }
}
