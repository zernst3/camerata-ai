# Contributing to camerata

This document is the contribution guide for humans **and AI agents** authoring canonical rules in the camerata principle library. The schema in [`src/principle.rs`](src/principle.rs) covers field shapes, but a rule that satisfies the schema can still be a bad rule. This document describes what a *good* rule looks like beyond the schema, so an agent generating a rule from a prompt has the same target as a human writing one.

The PR linter at [`.github/workflows/lint-principles.yml`](.github/workflows/lint-principles.yml) enforces a subset of these conventions mechanically. The rest are reviewer-enforced and described here.

---

## What a rule is

A camerata canonical rule is a single architectural, process, or convention **commit** that a project has chosen to adopt. It states the choice, gives the reasoning, and lists the alternatives the team considered before choosing. The rule's `summary` is what the downstream AI coding agent reads as instruction at code-author time; the rest of the rule is curation-time scaffolding for the architect (see *Field audiences* below).

A rule is **not** a tutorial, a how-to, a code-style note, a piece of project-specific documentation, or a survey of a topic. Rules at those layers belong elsewhere (a code-style guide, a README, a wiki). camerata sits at the architectural-commitment layer — decisions where multiple defensible options exist and the project has picked one.

The four-question test for whether a topic should be a camerata rule:

1. **Is there a real choice to make?** If there is only one defensible path, the rule does not need to exist. The linter and compiler will catch the wrong path; camerata does not need to.
2. **Are the alternatives positions some team would actually defend?** If the listed alternatives are obvious strawmen, the rule reads as overclaim.
3. **Can the rule be cited by a code reviewer in a comment?** If it cannot be cited, it is too vague or too broad.
4. **Is the rule project-agnostic?** If the rule only makes sense inside one specific project, it belongs in that project's docs, not in camerata's shared library.

If any answer is no, do not author the rule.

---

## Schema reminders

Every TOML file under `principles/<domain>/` must satisfy the schema in [`src/principle.rs`](src/principle.rs). Required fields:

| Field | Type | Notes |
|---|---|---|
| `id` | string | `DOMAIN-CONCEPT-N` format, uppercase, unique across the entire library. |
| `title` | string | One-sentence statement of the commit. No semicolons or multi-clause titles. |
| `tag` | enum | `universal`, `stack`, or `choice`. |
| `domain` | string | `*` for universal rules; otherwise a capability (`sql`, `permissions`, `ui`) or stack profile (`rust`, `rust:dioxus`, `javascript:next`). Use the full name, not an abbreviation (see style rule on domain naming). |
| `layer` | enum | `universal`, `language`, `library`, or `framework`. |
| `enforcement` | enum | `prose`, `structured`, or `mechanical`. See the enforcement-levels section below. |
| `default` | bool | Required. Whether the rule auto-checks when its domain is selected. See the default-flag section below. |
| `summary` | string | The rule's directive. Emitted to AGENTS.md or CONVENTIONS.md. Read by the consumer agent at code-author time. Plain prose, no markdown formatting, no code blocks. |
| `why` | string | The reasoning behind the commit. **Architect-only; not emitted.** Answers *why*, not *what*. |
| `alternatives` | array of strings | At least one entry. **Architect-only; not emitted.** Real positions, not strawmen. |

Optional: `stance` (architect-only; used at curation time, not emitted), `emits` (declarative routing override), `choice` (required when `tag = "choice"`; architect-only).

The linter rejects schema violations on every PR. It also runs three content checks: id format, id uniqueness across the library, and a no-backticks check on title, summary, why, and alternatives.

---

## Field audiences

Camerata rules serve two distinct audiences with opposite needs. Every field below is designed for one audience or the other. Writing a field for the wrong audience is the most common precision error.

**Architect-facing fields** are read by the human curator or by an AI agent operating in curation mode. They live in the source TOML, render in the GUI's review pane and the CLI prompts, and inform the architect's decision about whether and how to adopt the rule. They never reach the consumer agent.

**Consumer-facing fields** are read by the AI coding agent at code-author time, from the emitted AGENTS.md or CONVENTIONS.md file in a project's working directory. The consumer agent does not see the source TOML, the alternatives, the why, the stance, the tag, or the choice block. It sees only what is rendered to the emit file.

The two audiences have opposite needs. The architect benefits from rich context: trade-offs, reasoning, alternatives, conditional applicability. The consumer agent needs strict, unambiguous, deterministic instruction: one directive, no interpretation surface, no opt-out paths.

| Field | Audience | Spirit | Voice |
|---|---|---|---|
| `id` | both | Stable identity citable in PR comments. | `DOMAIN-CONCEPT-N`, uppercase. |
| `title` | both | Single-commit headline of the directive. | Property-shaped, no semicolons, no multi-clause "and." |
| `tag` | architect | Classifies how the rule applies (universal / stack / choice). Drives selection UI. | Enum value. |
| `domain` | architect | Scope of applicability. Drives routing to per-domain output files. | String matching a domain or capability identifier. |
| `layer` | architect | Position in the architectural stack; drives emit ordering (universal first, framework last). | Enum value. |
| `enforcement` | architect | How the rule is verified (prose / structured / mechanical). Drives default emit target file. | Enum value. |
| `default` | architect | Whether projects adopting this domain ship the rule by default. | Bool. |
| `summary` | **consumer** | The default directive emitted to the consumer agent. Active as-written when the architect adopts the rule as-is. On `tag = "choice"` rules where the architect picks a non-default option, the selected alternative replaces the summary in the emit; the summary is then NOT emitted. | Single clear directive. Plain prose. No hedging, no opt-out paths, no "or you might" clauses. Property-shaped (per rule #7). |
| `why` | architect | Reasoning for the rule's existence, so a curator or reviewer can decide if the rule still applies as conditions change. | Explains why, does not restate what (rule #3). Does not introduce opt-out paths (rule #4). |
| `alternatives` | architect at curation time; **consumer at emit time if selected** | Real positions some team has defended instead of the rule's stance. Informs the architect's selection decision. When the architect picks one, the alternative's text replaces the summary in the emit and becomes the consumer agent's directive for the rest of the project's lifetime. | Two or three substantive entries, each with a one-line tradeoff parenthetical (rule #5). Each alternative must work as both architect-facing context AND a standalone consumer-facing directive, because selection promotes it to the emitted rule. |
| `stance` | architect | Author-side metadata about authority level (default / recommended / opinionated). Used during curation. | Enum value. |
| `emits` | architect | Optional declarative routing override that pins a rule to a specific output file or scope. | TOML inline table; explicit per-output entries. |
| `[choice]` block | architect | Required when `tag = "choice"`. Defines the prompt and the option list the architect picks from at curation time. The chosen option's text replaces the default summary in the emit. | TOML table with `prompt`, `options`, `default`. |

**The only fields the consumer agent ever sees: `id`, `title`, and the active directive.** The active directive is the `summary` by default. On `tag = "choice"` rules where the architect picked a non-default option, the active directive is the selected alternative's text and the summary is NOT emitted. Every other field is curation-time scaffolding the consumer never reads.

When authoring a rule (human or AI), the precision discipline is: ask "who reads this field?" before writing each one. If the answer is the architect, rich context is welcome. If the answer is the consumer agent, the field must be a single deterministic directive.

---

## Emit headers: self-describing files

Each emitted file (AGENTS.md, CONVENTIONS.md) begins with a markdown header that tells the consumer AI agent what the file is for, what to do with the directives below, and where its companion file fits. The header is generated by camerata's emit logic (`src/emit.rs::file_header`); authors of new rules do not need to add to it. The standard header applies to every emit regardless of which rules were selected.

**Why the headers exist.** A consumer agent may load AGENTS.md or CONVENTIONS.md without other context: it does not see the source TOML library, the architect's selection profile, or this CONTRIBUTING document. The emit file must be self-describing so the agent knows the file is authoritative, what enforcement category the rules belong to, and how the two files relate. Without the headers, the agent has rules but no context for how to treat them.

**What the headers commit to.**
- The file's authority level (architect-adopted, not advisory).
- The enforcement category (prose for AGENTS.md; structured plus mechanical for CONVENTIONS.md).
- The relationship to the companion file (the agent should read both; CONVENTIONS.md IDs are citable in commits).
- Regeneration discipline (the file is generated; edit the principle selection upstream to change it).

Per [[handling-scrutiny-on-ai-orchestrated-work]] logic in the broader camerata thesis, self-describing emit is part of the trust contract: the consumer agent should not need to consult external documentation to apply the rules correctly.

**To change the headers**, edit `file_header` in `src/emit.rs`. The text appears in every emitted AGENTS.md and CONVENTIONS.md from that point forward; existing emitted files in downstream projects continue to use whatever header was current at generation time until those projects regenerate.

---

## Style rules (reviewer-enforced)

### 1. Plain prose only in `summary`, `why`, and `alternatives`

The summary, why, and alternatives fields render in the GUI's right pane through a small markdown-lite parser that handles only `#` and `##` headings, `- ` bullets, and blank-line paragraph breaks. Code blocks, indented code samples, ASCII tables, and inline markdown formatting (bold, italics, links) do not render correctly. Do not use them.

If a rule needs to reference a specific function name, type name, or framework API, refer to it in prose ("the framework's signal hook," "the resource hook," "the call method") rather than in code formatting. The downstream emit is for an AI agent that knows the framework; it does not need a code sample to recognize what is being referenced.

**No backticks anywhere in content.** The linter rejects them. If a backtick appears, rewrite the phrase in prose.

### 2. Tight, single-commit titles

The title is a short statement of the commit. Avoid semicolons. Avoid multi-clause titles connected by "and." A title that reads as two commits should be two rules.

Good: `Repositories return domain types, never persistence representations`.
Less good: `Repositories return domain types; mappers handle the translation; persistence library upgrades stay scoped`.

### 3. The `why` is architect-facing only; it is not emitted

The `why` field is read by the human curator and by an AI agent operating in curation mode. It is not emitted to AGENTS.md or CONVENTIONS.md and is never seen by the consumer agent at code-author time.

The spirit of `why` is to explain why the rule was chosen, so a curator or reviewer can decide whether the rule still applies as conditions change. A good `why` answers "what would have to be true about this project for this rule to be the right commitment?" It does not restate what the rule says (that is the summary's job), and it does not introduce opt-out paths or conditional execution rules (if those are load-bearing for the rule's application, they belong in the summary as scope clauses per rule #4).

Good: "Per-request validation collapses three failure modes into one error path and lets the request handler trust the types it receives."
Less good: "The rule says validate at the boundary because we want to validate at the boundary."

If a piece of reasoning is load-bearing for the consumer agent's execution (e.g. "does not apply to documentation-only changes"), it is directive content. Move it into the summary as a scope clause; do not rely on the why to communicate it, because the consumer agent will never see it.

### 4. Summaries are directives; trade-off discussion belongs in alternatives

The `summary` field is the only load-bearing field the consumer agent ever reads (alongside `title` and `id`, which are short). It must read as a single clear directive: what the agent should do, full stop. Trade-off discussions, opt-out paths, conditional alternatives, and per-project variations all belong in architect-only fields, never in the summary.

A summary that describes both "the default" and "the conditions under which a project might opt out of the default" leaves the consumer agent ambiguous about which path to take at runtime. The architect chooses the path at curation time through the GUI or the CLI choice prompt; the consumer agent reads only the chosen path's directive. Mixing the two collapses the curation-time audience and the runtime audience into one ambiguous surface, and the consumer agent cannot tell which instruction is the active one.

**How "the chosen path" actually emits:** when the architect adopts the rule as written, the summary is emitted to AGENTS.md or CONVENTIONS.md as the directive. When the architect picks one of the `alternatives` instead (on `tag = "choice"` rules or via the GUI's per-rule alternative buttons), that alternative's text is emitted *in place of* the summary; the summary is not emitted at all. The consumer agent always sees exactly one directive per rule. The summary is the default; an alternative replaces it when selected. This same discipline therefore applies to alternatives: each one must read as a single clear directive on its own, because selection promotes it to the consumer agent's instruction (see rule #5).

Practical test: read the summary out loud as instructions to a junior engineer with no other context. If the junior asks "wait, do I do X or Y?" the summary is hedged. Rewrite it as a single directive and move the trade-off into `alternatives`.

See the *Field audiences* section above for the full per-field audience and spirit reference.

### 5. Alternatives are real, not strawmen; and each one must work as a directive on its own

Every rule's `alternatives` list should describe positions some real team has defended. If the listed alternatives are obviously wrong (caricatures, anti-patterns, or "do nothing"), the rule reads as overclaim. Two or three substantive alternatives are better than one obvious strawman.

Good: `single-page application with no server-rendered initial paint (wins on architectural simplicity; loses indexability on every public route)`.
Less good: `do not have a website (loses all users)`.

**Alternatives are dual-purpose.** At curation time they are architect-facing context: they show the human (or AI agent) picking the rule what other defensible positions exist, so the selection decision is informed. At emit time, if the architect picks one of them, that alternative's text is emitted to AGENTS.md or CONVENTIONS.md *in place of* the summary; the consumer agent then sees the alternative as the rule's directive, and the summary is not emitted at all.

This means each alternative must be authored to satisfy both audiences. The leading clause should read as a directive the consumer agent can act on (same standard as the summary, per rule #4); the parenthetical tradeoff is architect context that travels along with the directive when emitted.

### 6. No cross-domain references; cross-rule references discouraged

A rule must stand on its own without referencing rules in other domains. A `rust:dioxus` rule must not cite a `rust` rule by ID, because a consumer who selects `rust:dioxus` but not `rust` will see a dangling reference. Same for citing `permissions` from inside `ui`, or `iac` from inside `ci-cd`.

Cross-rule references *within* the same domain are tolerated but discouraged. If a rule needs to invoke a concept from another rule in the same domain, restate the concept inline rather than citing the ID. The rule should read naturally if read in isolation.

### 7. Rules describe properties, not procedures

A rule states what the architecture or codebase **should look like**, not what a team or agent **should do** or in what order. A rule that reads as a numbered build plan, a porting sequence, a phased rollout, or a step-by-step procedure is describing a task, not a principle, and belongs in project documentation (a README, a runbook, a planning doc) rather than in camerata.

The diagnostic test: take the verbs in the rule's summary and ask whether they describe a static property or a sequence of actions.

- Properties: "is," "has," "lives in," "composes," "carries," "uses," "returns."
- Procedures: "build," "first," "then," "before," "consult," "in this order," "in this phase."

A rule whose summary leans on the second set of verbs is almost always task-shaped. The architectural principle hiding inside it (if there is one) is what should be extracted, restated as a property, and shipped; the procedural wrapper should be discarded.

Concrete example:
- **Task-shaped (wrong):** "Build the primitive component library first, then port features. Build primitives in this order: layout, forms, feedback, navigation, overlays, display."
- **Property-shaped (right):** "Common UI elements have one canonical implementation in a primitives layer; views compose primitives rather than reinventing them."

The two describe the same architectural intent, but only the second one survives the test of "would this rule make sense to a project that is not currently in the middle of building primitives?" The first reads as a build plan; the second as an invariant the codebase always satisfies.

If a rule cannot be restated as a property without losing its substance, it is fundamentally a task description and does not belong in camerata.

### 8. No project-specific content

A camerata rule must make sense to a project that has never heard of any other project. Do not reference:

- Specific company or product names.
- Specific repository or branch names.
- Internal team conventions (admin UI flows, internal naming schemes, role taxonomies).
- Specific third-party libraries or CDNs from a particular project's stack.
- The author's own personal projects or codebases.

A rule that says "use the v2 component library" or "follow the AUTH-1 pattern" only makes sense inside the project where v2 and AUTH-1 exist. The same rule generalized — "use the project's component library," "carry capability flags on response objects" — works for everyone.

### 9. `default = true` vs `default = false`

The `default` flag controls whether the rule's checkbox auto-checks when its domain is selected. Use the following heuristic:

- `default = true` — any project adopting this domain should ship this rule. It is the architectural baseline, not an opt-in extra.
- `default = false` — an opinionated extra. The rule is valuable in some contexts and the wrong default in others (caching strategies, auto-merge, feature-flag wiring, etc.).

If unsure, default to `true`. The library exists to commit, not to hedge.

Meta-domain rules (in `howto` and `contributing`) use `default = false` because they are documentation, not adoptable conventions.

### 10. Enforcement levels: pick what is actually achievable

| Level | Meaning | Emit target |
|---|---|---|
| `prose` | The agent reads the rule and follows it. No automated check possible. | `AGENTS.md` |
| `structured` | A rule with a citable ID. Reviewers cite the ID in commits and PR descriptions. Verified by grep and review. | `CONVENTIONS.md` |
| `mechanical` | A rule that *can* be enforced by a deterministic tool (lint rule, hook, CI check, type-system constraint). | `CONVENTIONS.md` plus a directive for the agent to set up the mechanism. |

Claiming `mechanical` without committing to provide the mechanism is a hollow claim and degrades the level to `structured` in practice. Either provide the mechanism (or describe it concretely enough that the downstream agent can implement it) or downgrade.

### 11. Domain-selection metadata is maintainer-only

The list of domains that ship pre-selected when the GUI launches is the `DEFAULT_SELECTED_DOMAINS` const in [`src/lib.rs`](src/lib.rs). Currently only `*` (Universal) and `agentic` ship pre-selected. **A PR contributing a new rule, capability domain, or stack profile must not modify this const.** Promoting a domain into the default-selected set is a maintainer-only decision; the PR linter rejects any PR that modifies the relevant lines without an explicit maintainer override.

### 12. Domain names use the canonical full form of language and framework names

A domain name (capability or stack profile) appears on every camerata surface: the AGENTS.md and CONVENTIONS.md a consumer agent reads, the search bar an architect types into, the file path in the `principles/` tree, and the citation a code reviewer pastes into a PR. The name must read clearly on every surface without inside knowledge.

For language and framework names specifically, use the canonical full form. Write `javascript`, not `js`. Write `typescript`, not `ts`. Write `python`, not `py`. Stack profiles compose with `:` between the base and the specialization: `javascript:next`, not `js:next`.

Widely-established shorthand for capability domains is acceptable when the shorthand IS the canonical term in the field. `api-layer`, `ci-cd`, `iac`, `sql`, `ui` are fine because the abbreviation reads as a stable, unambiguous term. Language and framework names do not have this property: "JavaScript" reads naturally as the language; "JS" reads as shorthand that a reader has to decode.

Why: domain names age into infrastructure. A new contributor reading `js:next` has to know "js" means JavaScript and "next" means Next.js; reading `javascript:next` they do not. Camerata is a public library; the names are public surface. The four characters saved by abbreviating language names once at curation time cost clarity downstream forever.

Format: lowercase, hyphens for compound capability names (`api-layer`, `ci-cd`), colon (`:`) as the stack-profile separator. No underscores or camelCase. The rule ID prefix follows the domain in uppercase (`JAVASCRIPT-NEXT-...`, not `JS-NEXT-...`); if you rename a domain, the IDs of rules in it rename to match.

---

## v0.1 limitations and v0.2 roadmap

These are known gaps in v0.1.0 that are documented here so contributors understand what camerata does and does not commit to today, and so that future contributors do not work around the gaps in ways that conflict with planned v0.2 features.

**Emit is overwrite-only.** Running `camerata generate` against a directory that already contains AGENTS.md, CONVENTIONS.md, or camerata.lock fully replaces those files. There is no merge step. The GUI's Generate-confirm banner lists the files that would be overwritten so the user can cancel and back them up first; the CLI does not yet check, though it will in a follow-up. Hand edits to the emitted files are lost on regeneration. Workaround for v0.1: keep all rule changes in the principle library (not in the emitted file) and re-run camerata to pick them up. v0.2 will add an upsert path that preserves hand-edited regions where possible.

**No reverse-engineering of a profile from an existing repo.** Camerata cannot today read a target repo's AGENTS.md / CONVENTIONS.md / camerata.lock and reconstruct a Profile JSON from them. The lock file records installed IDs and content hashes, and we could in principle parse the emit files back into a profile, but two things make this nontrivial enough to defer: (1) recovering which alternative was chosen on a `tag = "choice"` rule requires text-matching the emitted body against each alternative, which is fragile to whitespace and casing changes, and (2) custom rules live in AGENTS.md under their `CUSTOM-name` headings and parsing them back into the Profile schema requires careful handling of edge cases. v0.2 will introduce a `camerata import` subcommand that performs this reconstruction with an explicit "best effort" caveat.

**Alternatives have no stable IDs.** The `alternatives` array on each principle is a list of free-text strings today. There is no way to refer to a specific alternative by ID in a commit message, a PR description, or a profile JSON. This is intentional for v0.1: alternatives are architect-only (not emitted), and the consumer of a stable alternative ID does not yet exist. v0.2 will likely add IDs, in a format chosen alongside the reverse-engineering import feature so the two designs reinforce each other. **Until that lands, do not invent your own ID conventions in the alternatives array** (no `"ALT-1: text..."` prefixes, no inline tables with custom `id` fields). New rules should keep the alternatives array as plain strings exactly as the existing rules do.

---

## Working with an AI agent on a rule batch

When an AI agent (a Claude Code session, an automated PR bot, or any other coding agent) is generating multiple rules in a batch, the agent should:

1. Read this document into its working context **before** generating any rule.
2. Read at least three existing rules in the target domain to absorb the voice, register, and length.
3. Run `cargo run --bin camerata-lint principles/` after each rule and before opening the PR. Schema violations and backtick violations are caught here.
4. Verify that each rule passes the four-question test in the *What a rule is* section above. A rule that does not pass the test should be dropped, not shipped.
5. Avoid generating a single huge rule that bundles several decisions. If the title needs `and` to be accurate, the rule is two rules.

---

## Local workflow

```sh
# After authoring or editing rules:
cargo run --bin camerata-lint           # schema + content checks
cargo run --bin camerata -- list        # smoke test that everything loads
cargo run --bin camerata-gui --features gui   # visual check in the GUI
```

For a full scaffold test:

```sh
rm -f sandbox/AGENTS.md sandbox/CONVENTIONS.md sandbox/camerata.lock
cargo run --bin camerata -- init --out sandbox --stack <your-domain> --defaults
```

The `sandbox/` directory is gitignored except for `.gitkeep`, so you can dry-run scaffolds locally without polluting version control.

---

## License

By contributing, you agree your contribution is dual-licensed under MIT and Apache-2.0, matching the project's overall license (see [`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE)).
