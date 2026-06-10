# Contributing to camerata

This document is the contribution guide for humans **and AI agents** authoring canonical rules in the camerata principle library. The schema in [`src/principle.rs`](src/principle.rs) covers field shapes, but a rule that satisfies the schema can still be a bad rule. This document describes what a *good* rule looks like beyond the schema, so an agent generating a rule from a prompt has the same target as a human writing one.

The PR linter at [`.github/workflows/lint-principles.yml`](.github/workflows/lint-principles.yml) enforces a subset of these conventions mechanically. The rest are reviewer-enforced and described here.

---

## What a rule is

A camerata canonical rule is a single architectural, process, or convention **decision** that a project resolves. It states the question, lists the defensible options, optionally flags one as the default, and gives the reasoning. The directive of the adopted option is what the downstream AI coding agent reads as instruction at code-author time; the rest of the rule is curation-time scaffolding for the architect (see *Field audiences* below).

A rule is **not** a tutorial, a how-to, a code-style note, a piece of project-specific documentation, or a survey of a topic. Rules at those layers belong elsewhere (a code-style guide, a README, a wiki). camerata sits at the architectural-commitment layer — decisions where multiple defensible options exist and the project has picked one.

The four-question test for whether a topic should be a camerata rule:

1. **Is there a real choice to make?** If there is only one defensible path, the rule does not need to exist. The linter and compiler will catch the wrong path; camerata does not need to.
2. **Are the options positions some team would actually defend?** If the listed options are obvious strawmen, the rule reads as overclaim.
3. **Can the rule be cited by a code reviewer in a comment?** If it cannot be cited, it is too vague or too broad.
4. **Is the rule project-agnostic?** If the rule only makes sense inside one specific project, it belongs in that project's docs, not in camerata's shared library.

If any answer is no, do not author the rule.

---

## Schema reminders

Every TOML file under `principles/<domain>/` must satisfy the schema in [`src/principle.rs`](src/principle.rs). Top-level required fields:

| Field | Type | Notes |
|---|---|---|
| `id` | string | `AREA-TOPIC-NUMBER` format, uppercase, unique across the entire library. See the id format section below. |
| `title` | string | One-sentence statement of the decision. No semicolons or multi-clause titles. |
| `tag` | enum | `universal` or `stack`. (`choice` was retired: every rule is now a decision with options.) |
| `domain` | string | `*` for universal rules; otherwise a capability (`sql`, `permissions`, `ui`) or stack profile (`rust`, `rust:dioxus`, `javascript:next`). Use the full name, not an abbreviation (see style rule on domain naming). |
| `layer` | enum | `universal`, `language`, `library`, or `framework`. |
| `enforcement` | enum | `prose`, `structured`, or `mechanical`. See the enforcement-levels section below. |
| `default` | bool | Required. Whether the rule auto-checks when its domain is selected. Distinct from `decision.default`, which names the adopted option. See the default-flag section below. |

The rule then carries a `[decision]` block and one or more `[[option]]` entries:

| Field | Type | Notes |
|---|---|---|
| `decision.question` | string | What is being decided. Names the decision, not the winner. **Architect-only; not emitted.** |
| `decision.default` | string (optional) | The option id adopted when the rule is taken as-is. **Omit it for a genuinely open decision** (route-to-human): the rule does not emit until the architect resolves it. |
| `decision.why` | string | The reasoning behind the decision. **Architect-only; not emitted.** Answers *why*, not *what*. |
| `option.id` | string | Slug-cased, unique within the rule. Citable; stored in profiles. |
| `option.label` | string | Short human label shown in selection UI. **Architect-only; not emitted.** |
| `option.directive` | string | The consumer-facing instruction emitted when this option is adopted. Plain prose, no markdown formatting, no code blocks, no opt-out paths. |
| `option.why` | string | Per-option rationale (why it is or is not the default). **Architect-only; not emitted.** |

Optional: `emits` (declarative routing override).

The linter rejects schema violations on every PR. It also runs content checks: id format, id uniqueness across the library, at least one option, non-empty option fields, option-id uniqueness within a rule, that `decision.default` (when present) names an existing option, and a no-backticks check on title, the decision fields, and every option field. It reports (without failing) the count of no-default rules.

---

## Id format: AREA-TOPIC-NUMBER

Every rule id has the shape `AREA-TOPIC-NUMBER`. The linter rejects anything that does not match it, and the format is the same one cited in PR comments, commit messages, and CONVENTIONS.md emit blocks.

- **AREA** names the cluster the rule belongs to: `RUST`, `ARCH`, `ORCH`, `PROC`, `SPIRIT`, `CAMERATA`, `JAVASCRIPT`, and so on. One short uppercase word.
- **TOPIC** names the concept inside the area: `DOMAIN`, `CONTEXT-OVERRIDE`, `STRICT-LAYERING`, `USER-GUIDE`, and so on. May itself contain dashes (so an id can have more than three segments).
- **NUMBER** is a trailing integer that disambiguates successive rules in the same topic, starting at `1`.

Mechanical rules the linter enforces:

- At least three dash-separated segments.
- Every segment is non-empty and contains only uppercase ASCII letters or digits.
- The final segment is all digits.

Valid: `RUST-DOMAIN-4`, `ARCH-IAC-1`, `ORCH-CONTEXT-OVERRIDE-1`, `CAMERATA-USER-GUIDE-1`, `UI-CONSENT-GATED-1`.

Invalid: `RUST-4` (collapses AREA and TOPIC), `rust-domain-4` (lowercase), `RUST_DOMAIN_4` (underscores), `RUST-DOMAIN` (no trailing number), `RUST-DOMAIN-1a` (mixed in last segment).

The three-segment floor is what carries the convention: two-segment shapes like `RUST-4` collapse AREA into TOPIC, which the library has never used. The linter rejects them so PRs cannot silently introduce a different shape.

---

## Field audiences

Camerata rules serve two distinct audiences with opposite needs. Every field below is designed for one audience or the other. Writing a field for the wrong audience is the most common precision error.

**Architect-facing fields** are read by the human curator or by an AI agent operating in curation mode. They live in the source TOML, render in the GUI's review pane and the CLI prompts, and inform the architect's decision about whether and how to adopt the rule. They never reach the consumer agent.

**Consumer-facing fields** are read by the AI coding agent at code-author time, from the emitted AGENTS.md or CONVENTIONS.md file in a project's working directory. The consumer agent does not see the source TOML, the decision block, the option labels or whys, the non-adopted options, the tag, or the domain. It sees only the adopted option's directive (plus the id and title).

The two audiences have opposite needs. The architect benefits from rich context: trade-offs, reasoning, the full option set, conditional applicability. The consumer agent needs strict, unambiguous, deterministic instruction: one directive, no interpretation surface, no opt-out paths.

| Field | Audience | Spirit | Voice |
|---|---|---|---|
| `id` | both | Stable identity citable in PR comments. | `AREA-TOPIC-NUMBER`, uppercase. |
| `title` | both | Single-decision headline. | Property-shaped, no semicolons, no multi-clause "and." |
| `tag` | architect | Classifies how the rule applies (universal / stack). Drives selection UI. | Enum value. |
| `domain` | architect | Scope of applicability. Drives routing to per-domain output files. | String matching a domain or capability identifier. |
| `layer` | architect | Position in the architectural stack; drives emit ordering (universal first, framework last). | Enum value. |
| `enforcement` | architect | How the rule is verified (prose / structured / mechanical). Drives default emit target file. | Enum value. |
| `default` | architect | Whether projects adopting this domain ship the rule by default. Distinct from `decision.default`. | Bool. |
| `decision.question` | architect | What is being decided. Names the decision, not the winner. | Single interrogative sentence. |
| `decision.default` | architect | The option id adopted as-is. Omit for a genuinely open decision (route-to-human). | Option id, or absent. |
| `decision.why` | architect | Reasoning for the decision, so a curator can decide if it still applies as conditions change. | Explains why, does not restate what (rule #3). Does not introduce opt-out paths (rule #4). |
| `option.id` | both | Stable per-option identity, citable; stored in profiles. | Slug-cased, unique within the rule. |
| `option.label` | architect | Short selection-UI label for the option. | A few words; no parenthetical tradeoffs. |
| `option.directive` | **consumer** | The instruction emitted when this option is adopted. | Single clear directive. Plain prose. No hedging, no opt-out paths, no "or you might" clauses. Property-shaped (per rule #7). |
| `option.why` | architect | Per-option rationale: why this option is or is not the default. | Explains why; no directive content. |
| `emits` | architect | Optional declarative routing override that pins a rule to a specific output file or scope. | TOML inline table; explicit per-output entries. |

**The only fields the consumer agent ever sees: `id`, `title`, and the adopted option's `directive`.** The adopted option is the one named by `decision.default`, or the one the architect picked. Every other field is curation-time scaffolding the consumer never reads.

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

### 1. Plain prose only in the decision and option fields

The `decision.why`, every `option.directive`, and every `option.why` render in the GUI's right pane through a small markdown-lite parser that handles only `#` and `##` headings, `- ` bullets, and blank-line paragraph breaks. Code blocks, indented code samples, ASCII tables, and inline markdown formatting (bold, italics, links) do not render correctly. Do not use them.

If a rule needs to reference a specific function name, type name, or framework API, refer to it in prose ("the framework's signal hook," "the resource hook," "the call method") rather than in code formatting. The downstream emit is for an AI agent that knows the framework; it does not need a code sample to recognize what is being referenced.

**No backticks anywhere in content.** The linter rejects them. If a backtick appears, rewrite the phrase in prose.

### 2. Tight, single-commit titles

The title is a short statement of the commit. Avoid semicolons. Avoid multi-clause titles connected by "and." A title that reads as two commits should be two rules.

Good: `Repositories return domain types, never persistence representations`.
Less good: `Repositories return domain types; mappers handle the translation; persistence library upgrades stay scoped`.

### 3. The `why` is architect-facing only; it is not emitted

The `why` field is read by the human curator and by an AI agent operating in curation mode. It is not emitted to AGENTS.md or CONVENTIONS.md and is never seen by the consumer agent at code-author time.

The spirit of `decision.why` is to explain why the decision matters and why the default was chosen, so a curator or reviewer can decide whether the rule still applies as conditions change. A good `why` answers "what would have to be true about this project for this default to be the right commitment?" It does not restate what the directive says (that is the directive's job), and it does not introduce opt-out paths or conditional execution rules (if those are load-bearing for the rule's application, they belong in the adopted option's directive as scope clauses per rule #4).

Good: "Per-request validation collapses three failure modes into one error path and lets the request handler trust the types it receives."
Less good: "The rule says validate at the boundary because we want to validate at the boundary."

If a piece of reasoning is load-bearing for the consumer agent's execution (e.g. "does not apply to documentation-only changes"), it is directive content. Move it into the adopted option's directive as a scope clause; do not rely on the why to communicate it, because the consumer agent will never see it.

### 4. Every option's directive is a single self-contained directive

Each `option.directive` is the only load-bearing field the consumer agent ever reads for that option (alongside `title` and `id`, which are short). It must read as a single clear directive: what the agent should do, full stop. Trade-off discussions, opt-out paths, conditional alternatives, and per-project variations all belong in architect-only fields (`decision.why`, `option.why`), never in a directive.

A directive that describes both "the default" and "the conditions under which a project might opt out of the default" leaves the consumer agent ambiguous about which path to take at runtime. The architect chooses the option at curation time through the GUI or the CLI prompt; the consumer agent reads only the adopted option's directive. Mixing the two collapses the curation-time audience and the runtime audience into one ambiguous surface, and the consumer agent cannot tell which instruction is the active one.

**How the adopted option emits:** the directive of the option named by `decision.default` (or the option the architect picked) is emitted to AGENTS.md or CONVENTIONS.md; no other option's directive is emitted. The consumer agent always sees exactly one directive per rule. Because any option can be the adopted one, this discipline applies uniformly to every option, not just the default.

Practical test: read each directive out loud as instructions to a junior engineer with no other context. If the junior asks "wait, do I do X or Y?" the directive is hedged. Rewrite it as a single directive and move the trade-off into `decision.why` or `option.why`.

See the *Field audiences* section above for the full per-field audience and spirit reference.

### 5. Options are real, not strawmen

Every rule's option list should describe positions some real team has defended. If the listed options are obviously wrong (caricatures, anti-patterns, or "do nothing"), the rule reads as overclaim. Two or three substantive options are better than one obvious strawman. (A no-default decision in particular should present genuinely competing positions, since the architect must choose among them.)

Good: `the application renders entirely client-side with no server-rendered initial paint`.
Less good: `do not have a website`.

Each option carries its own `directive` (the consumer-facing instruction) and its own `why` (the architect-facing rationale for why this option is or is not the default). No parenthetical tradeoffs, no "(wins on X; loses on Y)" annotations, no meta-commentary inside the directive — that reasoning lives in `option.why`. When an architect adopts an option, its directive becomes the consumer agent's instruction; parentheticals reading like architect notes would leak into the directive and confuse the agent.

**Practical test:** if an option were adopted today, would a consumer agent reading its directive know exactly what to do without parsing any parenthetical context? If not, the directive is not yet self-contained. Strip the parenthetical and fold its content into the option's `why`.

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

**Project CONTEXT also belongs out of the directive surface, not just project names.** Each option's `directive` describes an instruction. It does NOT describe the project context that motivated the directive, the operational workflow the directive plugs into, or the rule author's specific orchestration setup. Examples of context leaks that fail this rule even when no project is named:

- Examples in the summary that name SPECIFIC situations the author had in mind ("(when porting the Rust:Dioxus layer of agora.new...)", "(when our team's review cadence is asynchronous...)") — these encode the author's project context.
- Cross-rule references to other camerata rules by name inside the summary or alternatives ("...recorded in the auto-calls ledger," "...flagged per ORCH-FOO-1"). Rules must stand alone; if a concept from another rule is load-bearing, restate it inline as an abstract operational property, not as a citation.
- Operational vocabulary that assumes a specific workflow shape ("our weekly ledger review pass," "the track record of overrides this team has built," "the iteration cadence the routine runs at") — these encode the author's project's workflow rather than the rule's directive.

**Important distinction: ABSTRACT CATEGORY TAXONOMIES that apply across projects are NOT leaks** and are often helpful for giving the consumer agent operational calibration. A taxonomy like "(such as the rule's stance conflicting with project positioning, a downstream constraint defeating the rule's rationale, an ecosystem convention diverging from the rule's stance, or similar context-fit failures)" describes generic failure modes that apply in any industry, on any stack. A reader in healthcare, fintech, or game development would recognize all three as relevant. Taxonomies like this give the consumer agent a starting framework for "when does this rule fire?" without committing the agent to a closed checklist. Frame taxonomies as illustrative ("such as," "or similar"), never as exhaustive ("if and only if").

The test: would a reader in a different industry on a different stack recognize the categories as relevant to their work? If yes, the taxonomy is fine and may belong in a directive as calibration. If no, the categories are project-specific and belong out of the rule entirely (or, if architectural-pattern reasoning rather than operational detail, in `decision.why`).

The `why` is the field where context for why the rule exists is welcome, and even there the context is *architectural reasoning* (what failure mode the rule prevents, what trade-off the rule resolves), not *project-operational context* (how a specific project's review process works, what cadence the author's team operates at). Rules are directives. The `why` exists to explain why the directive holds, not to describe the situation the author extracted the directive from.

The full diagnostic: read each option's directive out loud as if reading it to a team in a different industry, with a different review cadence, on a different stack. If a phrase only makes sense because the reader shares the author's project context, the phrase is a context leak and belongs in `decision.why` or out of the rule entirely. If a phrase reads as a generic category that any team would recognize, it is calibration, not a leak.

### 9. `default = true` vs `default = false`

The `default` flag controls whether the rule's checkbox auto-checks when its domain is selected. Use the following heuristic:

- `default = true` — any project adopting this domain should ship this rule. It is the architectural baseline, not an opt-in extra.
- `default = false` — an opinionated extra. The rule is valuable in some contexts and the wrong default in others (caching strategies, auto-merge, feature-flag wiring, etc.).

If unsure, default to `true`. The library exists to commit, not to hedge.

Meta-domain rules (in `howto` and `contributing`) use `default = false` because they are documentation, not adoptable conventions.

This top-level `default` bool is distinct from `decision.default`. The bool controls whether the rule is *checked at all* when its domain is selected; `decision.default` names *which option* the rule adopts when it is checked. A rule can be `default = true` (ship it) while having no `decision.default` (the architect must still pick which option to ship).

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

**v0.2 upsert design constraint (locked 2026-06-01): user-customized state is sacred.** The upsert merges upstream library updates into the project's existing emit; it does NOT overwrite any state the architect customized. The sacred set:

- `Profile.chosen` — the option ids the architect selected on each rule
- `Profile.custom_alternatives` — custom options the architect added to canonical rules
- `Profile.custom_rules` — rules the architect authored outside the canonical library
- `Profile.custom_domains` — domains the architect created
- Edited directive text (whether captured through a future GUI edit flow or hand-edited in the emit files)
- Recorded waivers (per the waiver mechanism in [[ORCH-CONFORMANCE-1]])

When the upsert encounters a conflict between an upstream library update and a user-customized rule, it surfaces a review prompt rather than silently overwriting. This connects to ORCH-CONTEXT-OVERRIDE-1: that rule requires explicit human sign-off before the agent overrides a documented rule. The upsert preservation contract is what makes that sign-off durable — once a human has signed off on a project-specific override, the override survives every subsequent regeneration. Without the preservation contract, the sign-off has no durability and the rule degrades into "pause now, lose the work later." The two are halves of the same architectural commitment.

**No reverse-engineering of a profile from an existing repo.** Camerata cannot today read a target repo's AGENTS.md / CONVENTIONS.md / camerata.lock and reconstruct a Profile JSON from them. The lock file records installed IDs and content hashes, and we could in principle parse the emit files back into a profile, but two things make this nontrivial enough to defer: (1) recovering which option was chosen requires text-matching the emitted body against each option's directive, which is fragile to whitespace and casing changes, and (2) custom rules live in AGENTS.md under their `CUSTOM-name` headings and parsing them back into the Profile schema requires careful handling of edge cases. v0.2 will introduce a `camerata import` subcommand that performs this reconstruction with an explicit "best effort" caveat.

**Stable option IDs and per-option `why` are now part of the schema.** The two former roadmap items (stable alternative IDs and a per-alternative `why` field) shipped together in the decision-first schema: every option carries a stable `id` (citable in commits and stored in profiles) and its own `why` alongside its `directive`. The rule-level reasoning now lives in `decision.why`, and per-option rejection rationale lives in each `option.why`. Author new rules directly in this shape; there is no longer an alternatives array.

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
