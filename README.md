# camerata

> Compose AI-orchestration principles into installable guardrails for your repos.

camerata is a small Rust tool (CLI + Dioxus Desktop GUI) that turns a curated library of AI-orchestration principles into the config files your AI coding tools actually read. Pick the principles that apply to your project, optionally add your own, route each domain to one or more target repos, and generate.

> **AI tool support (v0.1):** camerata emits to the **[AGENTS.md](https://agents.md/)** open standard (the file lives at the repo root). AGENTS.md is the cross-tool format read by Claude Code, Cursor, Codex, Copilot, Sourcegraph, and others, so a single emit covers them all out of the box. Structured rules (with rule IDs) also land in `CONVENTIONS.md` for grep + commit citation. The emit architecture is tool-agnostic; legacy `CLAUDE.md` or `.cursorrules` adapters can be added when there's demand.

## Why

AI-assisted development is fast at producing code and bad at remembering architecture. Without a written, citable set of conventions installed in the repo, every session re-derives the project's shape from scratch and accumulates inconsistencies that turn into architectural debt fast.

camerata is the curated, composable, layered alternative: assemble the principles that apply once, version them in your repo where the AI can actually read them, and refresh them as upstream conventions evolve.

## Getting started

### Prerequisites

camerata is written in Rust. You'll need:

- **Rust toolchain.** Install with [rustup](https://rustup.rs/) (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
- **A C/C++ toolchain.** Xcode CLI tools on macOS (`xcode-select --install`), `build-essential` on Linux, MSVC on Windows. The GUI's webview deps need it.
- **Linux only:** `libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libssl-dev` (or your distro's equivalents).

### Clone and build

```sh
git clone https://github.com/<your-username>/camerata-ai
cd camerata-ai
cargo build                                   # CLI only
cargo build --bin camerata-gui --features gui # CLI + GUI
```

### Run

```sh
cargo run --bin camerata-gui --features gui   # GUI (Dioxus Desktop)
cargo run -- list                              # browse every principle in the library
cargo run -- init --stack rust --defaults     # CLI: scaffold the default Rust set
```

The CLI also accepts `--out-domain DOMAIN=PATH` (repeatable; repeat the same domain to fan out to multiple repos) and `--json` to export your resolved decisions.

## Using the GUI

The GUI ships with an in-app user guide: open `camerata-gui` and the **Camerata · how to use** section is pinned to the top of the sidebar. Click the single entry inside it ("Camerata user guide") to read the same workflow + button reference shown below, rendered in the right pane with headings and bullets.

A typical session in `camerata-gui` looks like this:

1. **Launch.** Domain groups appear in the left column. The contributing section ("Camerata · how to contribute a canonical rule") is pinned to the top, followed by Universal, capability domains, and stack profiles. Universal and Choice rules are pre-selected by default.
2. **Browse and pick.** Click a domain to expand it. Click a principle title to read the full text, the rationale, and the alternatives in the right column. Tick the checkbox to include the rule; untick to skip it. For "choice"-tagged rules, the right column offers a button per option so you can pick a non-default alternative.
3. **Add your own where the library doesn't cover it.** **+ custom rule** under any domain attaches a user-authored rule to that domain. **+ Custom domain** at the bottom of the sidebar creates a new domain that holds only your own rules. "Add your own alternative" inside any canonical rule's detail pane records a context-specific variant on a built-in rule.
4. **Choose where it lands.** Type a path in **Output**, or click **Pick…** to browse, to set the default output directory. If you want to route different domains to different repos, open **Targets…** and configure the matrix.
5. **Save or generate.** **Save Profile…** writes the current selection to a JSON file you can reload later. **Generate** opens a confirm banner offering a save-then-generate path or a generate-only path; either way the in-progress autosave is cleared once the files are written.

### Action bar reference

| Button | What it does |
|---|---|
| **Output** | Editable text field showing the default output directory for the scaffold. Domains not mapped to a specific repo in Targets fall back here. |
| **Pick…** | Opens a native folder picker. The chosen folder fills the Output field. Pick only sets the *single default output*; it does not affect multi-repo routing. |
| **Generate** | Opens the Generate-confirm banner. Inside the banner you can save the profile first or skip straight to generating; either path clears the in-progress autosave after success. |
| **Save Profile…** | Writes the current selection to a profile JSON keyed by canonical-rule **id** (not by content), plus any custom rules, custom domains, and routing settings in full. Reload later with Load Profile. Because canonical rules are stored by id, future updates to a canonical rule's text are picked up automatically when the profile is loaded. |
| **Load Profile…** | Loads a profile JSON. Selections, choices, custom-alternative content, custom rules, custom domains, output dir, and target repos are restored. If the profile references canonical-rule ids that no longer exist in the current library, a red banner lists them and they're silently skipped. |
| **Export JSON…** | Exports the full content of the selected principles as JSON (id + title + summary + why + alternatives + chosen option). Use this for interchange with external systems that need the content embedded. Distinct from Save Profile: Save Profile is reference-only and intra-camerata; Export JSON is content-embedded and external. |
| **Targets…** | Toggles the Target Repos panel. Use it when you want to route different domains to different repos. If you only have one target, Output is enough and Targets is not needed. |
| **Exit** | Opens the exit-confirm prompt (Save and exit / Discard and exit / Cancel). The OS window close button triggers the same prompt. Save and exit writes a profile then clears the autosave; Discard and exit clears the autosave without saving; Cancel keeps the session running. The OS close path includes a brief window flash; the Exit button path does not. |

### Pick vs. Targets (the two are easy to confuse)

- **Output + Pick** is one place to send the scaffolded files. Domains with no per-repo mapping go here. Pick is just a folder browser for the Output text field.
- **Targets** is multi-repo routing. You add multiple repos, then for each domain decide which repos (one or more) should receive that domain's rules. Domains you don't map fall back to the single Output value.

If you're scaffolding into one project, you only need Output. Targets is for the case where camerata is the source of truth across multiple repos.

### Autosave and recovery

The GUI continuously writes the in-progress selection to a platform-standard temp file (`~/Library/Application Support/camerata/in-progress.json` on macOS, equivalent paths on Linux and Windows). If the app or your machine crashes, the next launch shows a yellow recovery banner offering **Resume** (restore the prior state) or **Start over** (discard it). A successful Generate clears the temp file because the work has been committed; the recovery banner only shows when there is genuinely unsaved work to recover.

For the exhaustive UX trace (every button, banner, and modal, numbered for bug reports), see [CAMERATA-QA.md](CAMERATA-QA.md).

## Concepts

- **Universal principles** apply to any AI-assisted project: the "spirit" philosophy (robustness, document decisions, optimization-by-default) and the decision-making heuristics for AI agents (clear-winner test, one-way-door routing, novelty downgrades authority).
- **Capability domains** are cross-cutting traits a project opts into, orthogonal to language: `sql`, `permissions`, `iac`, `api-layer`, `agentic` (autonomous-routine mechanics), `ci-cd`, `concurrency`. A SQL rule applies whether the project is in Rust, .NET, or anything else, the project just opts into the `sql` capability.
- **Stack profiles** are language/library/framework specific. The included `rust` profile has nested sub-domains for `seaorm`, `dioxus`, and `axum`.
- **Output routing** maps each selected domain to one or more target repos via a many-to-many matrix in the GUI (or `--out-domain` on the CLI). The same rule can fan out to several repos, and an unmapped domain falls back to the default output.
- **Custom rules** (and **custom alternatives** on built-in rules) let you extend the library per-project without forking it. **Custom domains** group user-authored rules together when they don't fit any canonical domain. **Profiles** save a session by canonical-rule id, so library updates to a rule's text flow through automatically on the next load.

## Anatomy of a principle

Each principle is a small TOML file in `principles/<domain>/`. The schema:

**Required**
- `id` — unique identifier (e.g. `RUST-DOMAIN-2`)
- `title` — short human-readable summary
- `tag` — `universal` (always on), `stack` (gated by domain selection), `choice` (prompts the user with options + a default)
- `domain` — `*` for universal, otherwise a stack or capability name (`rust`, `rust:seaorm`, `sql`, `permissions`, …)
- `layer` — `universal | language | library | framework` (precedence order; more-specific layers win conflicts)
- `enforcement` — declares how strongly the rule CAN be enforced once installed, and signals the AI agent what kind of artifact to produce:
  - **`prose`** — guidance the agent reads and follows; no automated check. Installed as a paragraph in AGENTS.md / the aicodingrules file. Best for heuristics (robustness over terseness, the clear-winner test).
  - **`structured`** — a rule with an ID that gets cited in commits, PRs, and review comments. Installed as an entry in CONVENTIONS.md. Verified by grep and review, not an automated gate. Best for architectural patterns.
  - **`mechanical`** — a rule that *can* be enforced by a deterministic tool (lint, hook, CI check, type-system constraint). Installed as the convention text **plus a directive to the agent to set up the actual mechanism** (write the lint, add the GitHub Action, configure the type pattern).

  **Important:** declaring a rule `mechanical` does NOT create the lint rule or workflow. It is a directive to the AI agent (or developer) to implement that enforcement when scaffolding the project. The principle text describes what the mechanism should check; wiring it up is still real work. A rule marked `mechanical` with no mechanism actually installed is degraded to `structured` in practice.
- `summary` — the rule itself, in one or two sentences
- `why` — the reasoning behind the chosen default
- `alternatives` — list of considered alternatives, each one line, with the tradeoff in parens

**Optional**
- `stance` — `default | recommended | opinionated`
- `emits` — list of `{ target, scope? }` declaring where this rule's artifact lands
- `choice` — `{ prompt, options, default }`, required when `tag = "choice"`

Example:

```toml
id = "RUST-DOMAIN-2"
title = "Newtype IDs (every ID is a wrapper, never a bare Uuid)"
tag = "stack"
domain = "rust"
layer = "language"
enforcement = "structured"
summary = "UserId(Uuid), OrganizationId(Uuid); bare Uuid only at persistence and HTTP boundaries."
why = "Moves wrong-ID-to-wrong-function bugs from runtime to compile time."
alternatives = [
    "bare Uuid everywhere (no safety)",
    "generic EntityId<T> (no runtime distinction)",
]
```

**Style:** keep each rule's text inside its own domain. No cross-domain analogies (a Dioxus rule must not reference a JavaScript library; a Rust async rule must not invoke a JS mental model). Meta-comments about applicability ("the idea is universal") are fine; what's forbidden is naming a *specific* other domain or language.

The same schema is mirrored as a principle inside the tool: open the GUI and the **Camerata · how to contribute a canonical rule** group is pinned to the top of the nav.

## A note on the principle library

The principles in this repo (especially the Rust profile) reflect conventions chosen during an in-progress port of a real production app. They are example content, not canonical authority. PRs that refine rules, correct mistakes, propose new capability domains, or add stack profiles are exactly what this project is for.

Every rule's `alternatives` field exists precisely so a reader can disagree with the chosen default and pick another option, without forking.

## Contributing

Principles live in `principles/<domain>/*.toml`, with rust sub-domains nested as `principles/rust/<framework>/`. To contribute a rule, add a file in the right folder and open a PR. New capability domains and stack profiles are also in scope.

## License

Dual-licensed under either:
- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

at your option.
