# Contributing to camerata

This document is the contribution guide for humans **and AI agents** authoring canonical rules in the camerata principle library. The schema in [`src/principle.rs`](src/principle.rs) covers field shapes, but a rule that satisfies the schema can still be a bad rule. This document describes what a *good* rule looks like beyond the schema, so an agent generating a rule from a prompt has the same target as a human writing one.

The PR linter at [`.github/workflows/lint-principles.yml`](.github/workflows/lint-principles.yml) enforces a subset of these conventions mechanically. The rest are reviewer-enforced and described here.

---

## What a rule is

A camerata canonical rule is a single architectural, process, or convention **commit** that a project has chosen to adopt. It states the choice, gives the reasoning, and lists the alternatives the team considered before choosing. The rule is what the downstream AI coding agent reads as instruction.

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
| `domain` | string | `*` for universal rules; otherwise a capability (`sql`, `permissions`, `ui`) or stack profile (`rust`, `rust:dioxus`, `js:next`). |
| `layer` | enum | `universal`, `language`, `library`, or `framework`. |
| `enforcement` | enum | `prose`, `structured`, or `mechanical`. See the enforcement-levels section below. |
| `default` | bool | Required. Whether the rule auto-checks when its domain is selected. See the default-flag section below. |
| `summary` | string | The rule's body. Plain prose, no markdown formatting, no code blocks. |
| `why` | string | The reasoning behind the commit. Answers *why*, not *what*. |
| `alternatives` | array of strings | At least one entry. Real positions, not strawmen. |

Optional: `stance` (used at curation time, not emitted), `emits` (declarative routing override), `choice` (required when `tag = "choice"`).

The linter rejects schema violations on every PR. It also runs three content checks: id format, id uniqueness across the library, and a no-backticks check on title, summary, why, and alternatives.

---

## Style rules (reviewer-enforced)

### 1. Plain prose only in `summary`, `why`, and `alternatives`

The summary, why, and alternatives fields render in the GUI's right pane through a small markdown-lite parser that handles only `#` and `##` headings, `- ` bullets, and blank-line paragraph breaks. Code blocks, indented code samples, ASCII tables, and inline markdown formatting (bold, italics, links) do not render correctly. Do not use them.

If a rule needs to reference a specific function name, type name, or framework API, refer to it in prose ("the framework's signal hook," "the resource hook," "the call method") rather than in code formatting. The downstream emit is for an AI agent that knows the framework; it does not need a code sample to recognize what is being referenced.

**No backticks anywhere in content.** The linter rejects them. If a backtick appears, rewrite the phrase in prose.

### 2. No em-dashes or en-dashes

Use commas, colons, periods, parentheses, or restructured sentences instead. Hyphens in compound words (`single-page application`, `state-of-the-art`) are fine; only true em-dashes (`—`) and en-dashes (`–`) are banned. The linter does not enforce this today but is planned to.

### 3. Tight, single-commit titles

The title is a short statement of the commit. Avoid semicolons. Avoid multi-clause titles connected by "and." A title that reads as two commits should be two rules.

Good: `Repositories return domain types, never persistence representations`.
Less good: `Repositories return domain types; mappers handle the translation; persistence library upgrades stay scoped`.

### 4. The `why` answers *why*, not *what*

A common failure mode is restating the rule in the why field. The why exists so the downstream agent can apply judgment at edge cases: the agent knows the rule, and the why tells it the rule's purpose so it can extrapolate when the rule does not cleanly apply. If the why is just a longer version of the summary, rewrite it.

Good: `Per-request validation collapses three failure modes into one error path and lets the request handler trust the types it receives.`
Less good: `The rule says validate at the boundary because we want to validate at the boundary.`

### 5. Alternatives are real, not strawmen

Every rule's `alternatives` list should describe positions some real team has defended. If the listed alternatives are obviously wrong (caricatures, anti-patterns, or "do nothing"), the rule reads as overclaim. Two or three substantive alternatives are better than one obvious strawman.

Good: `single-page application with no server-rendered initial paint (wins on architectural simplicity; loses indexability on every public route)`.
Less good: `do not have a website (loses all users)`.

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
