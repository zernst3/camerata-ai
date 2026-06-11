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
git clone https://github.com/zernst3/camerata-ai
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

## Using the CLI

The CLI is a thin frontend over the same library the GUI uses. It is the right surface for CI, scripted scaffolds, and headless environments. Profile save/load, custom rules, and custom domains are GUI-only in v0.1; the CLI focuses on scaffolding the canonical library.

### Commands

| Command | What it does |
|---|---|
| `camerata list` | Print every principle in the library (id, domain, tag, title). Useful for browsing what's available. |
| `camerata init` | Scaffold selected principles into a target directory. See the flag reference below. |
| `camerata export` | Print the entire principle library as a JSON catalog on stdout. For piping into other tools. |
| `camerata outdated` | Read this project's `camerata.lock` and report installed rules whose upstream content has changed, or that no longer exist. Pass `--dir PATH` to check a different directory. |

### `camerata init` flags

| Flag | Purpose |
|---|---|
| `--out PATH` | Default output directory (defaults to `.`). Unmapped domains land here. |
| `--out-domain DOMAIN=PATH` | Per-domain routing. Repeatable; repeat the same domain to fan it out to multiple repos. e.g. `--out-domain rust=../rust-repo --out-domain sql=../db`. |
| `--stack DOMAIN` | Include the named domain (a stack like `rust`, `ts:next`, or a capability like `sql`, `permissions`). Repeatable. Universal (`*`) and `agentic` are always included; this flag adds to them. |
| `--defaults` | Skip all prompts and take each rule's default option. Non-interactive mode; rules with no default (route-to-human) are skipped and reported. |
| `--all` | Include every rule in the selected domains, not just the ones marked `default = true`. Opt in to the opinionated extras (caching strategies, auto-merge, feature-flag wiring, etc.) alongside the core defaults. |
| `--minimal` | Drop the Universal layer entirely. Rare; mostly used when you only want stack-specific guardrails. |
| `--json` | Also write `camerata.selections.json` next to the scaffold (a machine-readable record of your decisions). |
| `--principles PATH` | (Top-level flag) Use a different principles directory. Useful when developing your own library locally. |

### Recipes

Default minimal scaffold (just Universal + Agentic, the always-on baseline):

```sh
camerata init --out ./agents --defaults
```

Rust project with default Rust rules:

```sh
camerata init --out . --stack rust --defaults
```

Full kitchen-sink Rust + SQL with the opinionated extras included:

```sh
camerata init --out . --stack rust --stack sql --defaults --all
```

Monorepo fan-out, one repo per stack:

```sh
camerata init \
  --stack rust --stack ts:next --stack sql \
  --out-domain rust=./backend \
  --out-domain ts:next=./web \
  --out-domain sql=./db \
  --defaults
```

### CLI vs GUI in v0.1

| Feature | CLI | GUI |
|---|---|---|
| Scaffold canonical library | ✓ | ✓ |
| Multi-repo routing | ✓ | ✓ |
| `default` flag honored | ✓ | ✓ |
| `DEFAULT_SELECTED_DOMAINS` honored | ✓ | ✓ |
| Save / Load Profile | — | ✓ |
| Custom rules / custom domains | — | ✓ |
| Autosave + crash recovery | — | ✓ |
| Guided exit prompt | — | ✓ |

Profile and custom-rule support on the CLI is planned for v0.2. The likely shape is a `--profile <path>` flag that loads a profile JSON written by the GUI, plus a `--save-profile <path>` flag that captures the current run's choices.

## Using the GUI

The GUI ships with an in-app user guide: open `camerata-gui` and the **Camerata · how to use** section is pinned to the top of the sidebar. Click the single entry inside it ("Camerata user guide") to read the same workflow + button reference shown below, rendered in the right pane with headings and bullets.

A typical session in `camerata-gui` looks like this:

1. **Launch.** Domain groups appear in the left column. The contributing section ("Camerata · how to contribute a canonical rule") is pinned to the top, followed by Universal, capability domains, and stack profiles. Universal and Agentic rules are pre-selected by default.
2. **Browse and pick.** Click a domain to expand it. Click a principle title to read the decision question, the rationale, and the option list in the right column. Tick the checkbox to include the rule; untick to skip it. The detail pane offers a button per option with the default pre-selected, so you can adopt a different option. A rule with no default shows "requires your choice" until you pick one.
3. **Add your own where the library doesn't cover it.** **+ custom rule** under any domain attaches a user-authored rule to that domain. **+ Custom domain** at the bottom of the sidebar creates a new domain that holds only your own rules. "Add your own option" inside any canonical rule's detail pane records a context-specific option on a built-in rule.
4. **Choose where it lands.** Type a path in **Output**, or click **Pick…** to browse, to set the default output directory. If you want to route different domains to different repos, open **Targets…** and configure the matrix.
5. **Save or generate.** **Save Profile…** writes the current selection to a JSON file you can reload later. **Generate** opens a confirm banner offering a save-then-generate path or a generate-only path; either way the in-progress autosave is cleared once the files are written.

### Action bar reference

| Button | What it does |
|---|---|
| **Output** | Editable text field showing the default output directory for the scaffold. Domains not mapped to a specific repo in Targets fall back here. |
| **Pick…** | Opens a native folder picker. The chosen folder fills the Output field. Pick only sets the *single default output*; it does not affect multi-repo routing. |
| **Generate** | Opens the Generate-confirm banner. Inside the banner you can save the profile first or skip straight to generating; either path clears the in-progress autosave after success. |
| **Save Profile…** | Writes the current selection to a profile JSON keyed by canonical-rule **id** (not by content), plus any custom rules, custom domains, and routing settings in full. Reload later with Load Profile. Because canonical rules are stored by id, future updates to a canonical rule's text are picked up automatically when the profile is loaded. |
| **Load Profile…** | Loads a profile JSON. Selections, chosen options, custom-option content, custom rules, custom domains, output dir, and target repos are restored. Legacy profiles that stored the chosen option as full text are best-effort upgraded to option ids on load. If the profile references canonical-rule ids that no longer exist in the current library, a red banner lists them and they're silently skipped. |
| **Export JSON…** | Exports the full content of the selected principles as JSON (id + title + decision + options + chosen option). Use this for interchange with external systems that need the content embedded. Distinct from Save Profile: Save Profile is reference-only and intra-camerata; Export JSON is content-embedded and external. |
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
- **Custom rules** (and **custom options** on built-in rules) let you extend the library per-project without forking it. **Custom domains** group user-authored rules together when they don't fit any canonical domain. **Profiles** save a session by canonical-rule id, so library updates to a rule's text flow through automatically on the next load.

## Anatomy of a principle

Each principle is a small TOML file in `principles/<domain>/`. The schema:

A rule models one architectural **decision** with a uniform list of **options**. The directive of the adopted option is the only thing the downstream agent reads; everything else is curation-time context.

**Top-level required**
- `id` — unique identifier (e.g. `RUST-DOMAIN-2`)
- `title` — short human-readable headline of the decision
- `tag` — `universal` (always on) or `stack` (gated by domain selection)
- `domain` — `*` for universal, otherwise a stack or capability name (`rust`, `rust:seaorm`, `sql`, `permissions`, …)
- `layer` — `universal | language | library | framework` (precedence order; more-specific layers win conflicts)
- `default` — bool; whether the rule auto-checks when its domain is selected
- `enforcement` — declares how strongly the rule CAN be enforced once installed, and signals the AI agent what kind of artifact to produce:
  - **`prose`** — guidance the agent reads and follows; no automated check. Installed as a paragraph in AGENTS.md / the aicodingrules file. Best for heuristics (robustness over terseness, the clear-winner test).
  - **`structured`** — a rule with an ID that gets cited in commits, PRs, and review comments. Installed as an entry in CONVENTIONS.md. Verified by grep and review, not an automated gate. Best for architectural patterns.
  - **`mechanical`** — a rule that *can* be enforced by a deterministic tool (lint, hook, CI check, type-system constraint). Installed as the convention text **plus a directive to the agent to set up the actual mechanism** (write the lint, add the GitHub Action, configure the type pattern). Every mechanical rule carries a `qualifies` conformance test (see Optional fields below) that emits as its `_Conformance:_` line.

  **Important:** declaring a rule `mechanical` does NOT create the lint rule or workflow. It is a directive to the AI agent (or developer) to implement that enforcement when scaffolding the project. The principle text describes what the mechanism should check; wiring it up is still real work. A rule marked `mechanical` with no mechanism actually installed is degraded to `structured` in practice.

**The `[decision]` block**
- `question` — what is being decided (names the decision, not the winner)
- `default` — the option id adopted as-is; **omit it for a genuinely open decision** (route-to-human: the rule does not emit until resolved)
- `why` — the reasoning behind the decision

**One or more `[[option]]` entries**
- `id` — slug-cased, unique within the rule; citable and stored in profiles
- `label` — short selection-UI label
- `directive` — the consumer-facing instruction emitted when this option is adopted (the only field the downstream agent reads)
- `why` — per-option rationale (why it is or is not the default)

**Optional**
- `qualifies` — a deterministic conformance test that proves adherence to the adopted directive: prose describing the check, or a runnable command (grep, clippy/eslint, CI, a test). **Required on `mechanical` rules**, where it emits as the rule's `_Conformance:_` line in CONVENTIONS.md; accepted but never emitted on `prose`/`structured` rules. This is the field that operationalizes `ORCH-CONFORMANCE-1` (a codified commitment is an enforced gate only when a deterministic check is named).
- `emits` — list of `{ target, scope? }` declaring where this rule's artifact lands

Example:

```toml
id = "RUST-DOMAIN-2"
title = "Newtype IDs (every ID is a wrapper, never a bare Uuid)"
tag = "stack"
domain = "rust"
layer = "language"
enforcement = "structured"
default = true

[decision]
question = "How are entity IDs represented in the type system?"
default = "newtype-ids"
why = "Moves wrong-ID-to-wrong-function bugs from runtime to compile time."

[[option]]
id = "newtype-ids"
label = "newtype wrappers per ID"
directive = "UserId(Uuid), OrganizationId(Uuid); bare Uuid only at persistence and HTTP boundaries."
why = "The adopted default. Each ID type is distinct at compile time, so a wrong-ID argument fails to typecheck."

[[option]]
id = "bare-uuid"
label = "bare Uuid everywhere"
directive = "IDs are bare Uuid values throughout the codebase."
why = "Simplest, but offers no compile-time protection against passing the wrong ID."

[[option]]
id = "generic-entity-id"
label = "generic EntityId<T>"
directive = "IDs use a single generic EntityId<T> wrapper parameterized by entity type."
why = "Typed at compile time but the runtime representation is uniform, so reflection and serialization cannot distinguish them."
```

**Style:** keep each rule's text inside its own domain. No cross-domain analogies (a Dioxus rule must not reference a JavaScript library; a Rust async rule must not invoke a JS mental model). Meta-comments about applicability ("the idea is universal") are fine; what's forbidden is naming a *specific* other domain or language.

The same schema is mirrored as a principle inside the tool: open the GUI and the **Camerata · how to contribute a canonical rule** group is pinned to the top of the nav.

## A note on the principle library

The principles in this repo (especially the Rust profile) reflect conventions chosen during an in-progress port of a real production app. They are example content, not canonical authority. PRs that refine rules, correct mistakes, propose new capability domains, or add stack profiles are exactly what this project is for.

Every rule's option list exists precisely so a reader can disagree with the chosen default and pick another option, without forking.

## Contributing

Principles live in `principles/<domain>/*.toml`, with rust sub-domains nested as `principles/rust/<framework>/`. To contribute a rule, add a file in the right folder and open a PR. New capability domains and stack profiles are also in scope.

### Pre-flight lint

A small Rust linter (`src/bin/lint.rs`, run as `cargo run --bin camerata-lint`) enforces the schema and content rules the Anatomy principle claims:

- Every file parses as TOML and matches the `Principle` schema (covers required fields, the `[decision]` block, the `[[option]]` list, `tag`/`layer`/`enforcement` enum values, and `default` is a bool).
- `id` matches the canonical format `AREA-TOPIC-NUMBER`.
- `id` is unique across the entire library.
- At least one `[[option]]`, every option has a non-empty id/label/directive/why, option ids are unique within the rule, and `decision.default` (when present) names an existing option.
- Every `mechanical` rule declares a non-empty `qualifies` conformance test.
- Title, the decision fields, and every option field contain no backtick characters.

Run the linter locally before opening a PR:

```sh
cargo run --bin camerata-lint
# or against a different principles dir:
cargo run --bin camerata-lint -- path/to/principles
```

The same linter runs on every PR via [`.github/workflows/lint-principles.yml`](.github/workflows/lint-principles.yml), along with a check that the PR has not modified `DEFAULT_SELECTED_DOMAINS` in `src/lib.rs` (that's a maintainer-only change per the Anatomy rule).

Substantive review — options being real positions some team defends, "why" answering why and not what, no cross-domain references — is handled by human reviewers today and is on the v0.2 roadmap for an LLM-assisted reviewer.

## License

Dual-licensed under either:
- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

at your option.
