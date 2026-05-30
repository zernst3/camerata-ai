# Camerata QA Regression Document

This document traces every piece of user-facing functionality in the Camerata GUI, in the order a user might encounter it. Use it to verify behavior after a change or to file a bug report against a specific item (cite the section number).

If something below doesn't match what you see, open an issue with the section number, what you expected, and what actually happened.

Tested against: `camerata-gui` v0.1.0 (Dioxus Desktop).

---

## 1. App launch

1.1 Launching `camerata-gui` opens a single window with the title bar showing the OS-native window chrome (no custom title bar).

1.2 The header reads **camerata** followed by a smaller grey subtitle: "· AI-orchestration scaffolder (preview)".

1.3 If a previous session was interrupted (the autosave file exists on disk), a yellow recovery banner appears beneath the header. See section 2.

1.4 If no previous session was interrupted, the recovery banner does not appear and the action bar is the first thing below the header.

---

## 2. Recovery banner (only when autosave file exists)

2.1 The banner reads: "Pick up where you left off? An unsaved in-progress profile was found."

2.2 The banner has two buttons: **Resume** and **Start over**.

2.3 Clicking **Resume** restores the prior session's state: selected canonical rules, choice picks, user-added alternatives, custom rules, custom domains, output dir, target repos, and per-domain repo routing. The banner disappears.

2.4 If the loaded recovery profile references canonical-rule ids that no longer exist in the current library (renamed or removed in a newer version), a red missing-ids banner appears below the recovery banner after Resume. See section 3.

2.5 Clicking **Start over** deletes the autosave file and dismisses the banner. The session continues with default state (all Universal and Choice canonical rules pre-selected, no customs, default output dir).

2.6 The autosave file path is platform-specific:
- macOS: `~/Library/Application Support/camerata/in-progress.json`
- Linux: `~/.local/share/camerata/in-progress.json`
- Windows: `%LOCALAPPDATA%\camerata\in-progress.json`

---

## 3. Missing-ids warning banner

3.1 The banner appears after a Resume or a Load Profile that references rule ids no longer in the library.

3.2 The banner reads: "Some rules from this profile are no longer in the library and were skipped:" followed by a bulleted list of the missing ids in monospace font.

3.3 The banner has a **Dismiss** button that hides it.

3.4 Selections, choices, and custom-alternative mappings for missing ids are silently dropped. All other state from the profile is applied normally.

---

## 4. Action bar

4.1 The action bar runs horizontally beneath the header (and banners, if present).

4.2 Left to right, the action bar contains:
- "Output:" label
- An editable text input showing the current output directory (default `/tmp/camerata-demo`)
- **Pick…** button (opens native folder picker)
- **Generate** button
- **Save Profile…** button
- **Load Profile…** button
- **Export JSON…** button
- **Targets…** button
- **Exit** button (opens the exit-confirm banner; see section 4B)

4.3 **Pick…** opens the OS-native folder picker. Choosing a folder fills the output input. Cancel leaves the input unchanged.

4.4 **Generate** opens the Generate-confirm banner (section 4A). The actual emit (scaffolded files written to the output directory or per-domain repos) happens from inside that banner so the user has a clear save-or-skip choice before the in-progress autosave is cleared.

4.5 **Save Profile…** opens a native save dialog with default filename `camerata.profile.json`. Saves a profile JSON file containing canonical-rule selections by id, choice picks by id, custom-alternative content by id, custom rules in full, custom domains, output dir, target repos, and per-domain routing. Profiles are resilient to library updates because they reference canonical content by id, not by snapshot.

4.6 **Load Profile…** opens a native open dialog filtered to `.json` files. On selection, applies the profile state. If any selected id is no longer in the library, the red missing-ids banner appears (section 3).

4.7 **Export JSON…** opens a save dialog with default filename `camerata.selections.json`. Exports the FULL content of every selected principle (id + title + summary + why + alternatives + chosen option) as JSON. Use this for interchange with external systems that need the rule content embedded. NOT to be confused with Save Profile — Export JSON is a content snapshot, Save Profile is an id-based reference.

4.8 **Targets…** toggles the Target Repos panel (section 5).

4.9 A status line appears in green below the action bar after any action that produced output (Save Profile, Load Profile, Export JSON, Generate). The status line persists until the next action replaces it.

---

## 4C. Delete-confirm banner

4C.1 Triggered by clicking the × button on:
- A custom rule row in the sidebar (right side of the row, hover-tooltip "Delete this custom rule")
- A custom domain header (right side of the row, hover-tooltip "Delete this custom domain and all its custom rules")

4C.2 Canonical-library rules and domains do NOT have a × button. Deletion only applies to user-authored items.

4C.3 Appears as a light-red banner between the action bar and the workspace columns.

4C.4 For a custom rule deletion:
- Header: "Delete custom rule \"<name>\"?"
- Body: "This cannot be undone."

4C.5 For a custom domain deletion:
- Header: "Delete custom domain \"<name>\"?"
- Body lists how many custom rules will also be deleted:
  - 0 rules: "This domain has no custom rules. This cannot be undone."
  - 1 rule: "Deleting this domain will also delete 1 custom rule scoped to it. This cannot be undone."
  - N rules: "Deleting this domain will also delete <N> custom rules scoped to it. This cannot be undone."

4C.6 Two buttons:
- **Delete** (red background): performs the deletion. For a domain, this cascades — all custom rules whose domain matches are removed, the domain is removed from the custom-domain list, the domain is removed from selected_domains, and the domain is removed from the expanded set.
- **Cancel**: dismisses the banner without deleting.

4C.7 The banner closes on either button. Reopening requires clicking the × button again.

4C.8 Autosave fires after the deletion completes (the autosave effect is subscribed to custom_rules / custom_domains), so the next launch reflects the deletion as in-progress state.

---

## 4B. Exit-confirm banner

4B.1 Triggered by either:
- Clicking the **Exit** button in the action bar
- Clicking the OS window close button (the red X on macOS, the X on Windows/Linux)

4B.2 When triggered by the OS close button, the runtime briefly hides the window before the prompt re-shows it. This produces a short visual flash on the OS close path only; the Exit-button path does not flash.

4B.3 Appears as a light-orange banner between the action bar and the workspace columns.

4B.4 Header text: "Save your work before closing?"

4B.5 Sub-text: "Closing the app will clear the in-progress autosave. Save a profile now to keep your selection, or discard and exit."

4B.6 Three buttons:
- **Save and exit**
- **Discard and exit**
- **Cancel**

4B.7 **Save and exit** opens a native save dialog with default filename `camerata.profile.json`.
- If the user picks a path and the save succeeds, the autosave temp file is deleted and the window closes (app exits).
- If the user cancels the save dialog, the banner stays open (they can pick again or choose Discard).
- If the save fails (disk full, permission denied), the error appears in the status line and the banner stays open.

4B.8 **Discard and exit** clears the autosave temp file and closes the window (app exits) without saving the in-progress selection.

4B.9 **Cancel** dismisses the banner. The session continues normally; the autosave is preserved and resumes mirroring state changes.

4B.10 Autosave lifecycle around exit:
- Save and exit: autosave deleted (it's now in the named profile file).
- Discard and exit: autosave deleted (user explicitly threw it away).
- Cancel: autosave preserved (the user backed out of closing).
- Window close from elsewhere (process kill, crash, power loss): autosave preserved, recovery banner appears on next launch.

## 4A. Generate-confirm banner

4A.1 Triggered by clicking **Generate** in the action bar. Appears as a light-blue banner between the action bar and the workspace columns.

4A.2 Header text: "Save your profile before generating?"

4A.3 Sub-text: "Generating will write the scaffolded files into the configured output. The in-progress autosave will be cleared either way."

4A.4 Three buttons:
- **Save profile and generate**
- **Generate without saving**
- **Cancel**

4A.5 **Save profile and generate** opens a native save dialog with default filename `camerata.profile.json`.
- If the user picks a path and the save succeeds, the scaffold emit runs immediately afterward. The status line reads: "Profile saved to <path>. Generated into <targets>". The autosave temp file is deleted. The banner closes.
- If the user cancels the save dialog, the banner stays open (user is in a "save first" mindset; they can pick a path or switch to "Generate without saving").
- If the save fails (disk full, permission denied), an error appears in the status line and the banner stays open.

4A.6 **Generate without saving** runs the scaffold emit immediately. The status line shows the targets and rule counts. The autosave temp file is deleted. The banner closes.

4A.7 **Cancel** closes the banner without generating or saving. The autosave temp file is NOT deleted (the in-progress state continues to autosave normally).

4A.8 Autosave lifecycle around this banner:
- Opening the banner does NOT clear the autosave.
- A successful generate via either of the two action buttons clears the autosave.
- A canceled banner (or a save-dialog cancel inside "Save profile and generate") leaves the autosave intact.

4A.9 Error cases:
- If the scaffold emit fails (output directory unwritable, internal error), the status line shows "Error: <message>" and the autosave is NOT deleted (the work is still in progress).

---

## 5. Target Repos panel (toggled by **Targets…**)

5.1 When closed: hidden.

5.2 When open: a bordered grey panel appears between the action bar and the two-column layout. Header text: "Target repos".

5.3 The panel contains:
- A text input for entering a repo path with a **Pick…** button (native folder picker)
- A **Create new repo** button to add the typed path
- A row of removable chips (one per added repo, each with a small × button)
- A per-domain repo mapping: for each domain (including custom domains), a row with the domain label and a checkbox per added repo

5.4 Adding a repo: type a path or pick a folder → click **Create new repo**. The repo appears as a chip.

5.5 Removing a repo: click the × on a chip. The repo is removed from the chip list and from any per-domain mapping.

5.6 Per-domain mapping: check a repo box next to a domain to route that domain's installed rules to that repo when **Generate** runs. Multiple repos can be mapped per domain. Unmapped domains use the default output dir from section 4.

5.7 If no repos are added, an italic grey tip reads: "Add or create a repo, then map domains to it. Unmapped domains use the default output above."

---

## 6. Two-column layout (main workspace)

6.1 Below the action bar (and Target panel if open), the screen splits into two columns.

6.2 Left column (~46% width): collapsible domain list with principles. Bordered on the right.

6.3 Right column (~54% width): context-dependent. Shows one of: principle detail, custom-rule form, custom-domain form, or empty-state tip.

6.4 A small "N selected" counter sits at the top-right of the left column, showing the number of currently-selected canonical rules.

---

## 7. Domain groups (left column)

7.1 Each domain appears as a collapsible bordered grey button with a disclosure triangle:
- "▾  " when expanded
- "▸  " when collapsed

7.2 Domain labels follow this format:
- "Camerata · how to use" for the `howto` meta-documentation domain
- "Camerata · how to contribute a canonical rule" for the `contributing` meta-documentation domain
- "Universal" for the `*` domain
- "Capability · <name>" for capability domains (sql, permissions, iac, api-layer, agentic, ci-cd, concurrency)
- "Stack · <name>" for stack domains (rust, ts, ts:next, ts:drizzle, rust:seaorm, etc.)
- The raw name for user-created custom domains

7.3 Meta-documentation domains (`howto` and `contributing`) are pinned to the top of the list, in that order: How To first, then How To Contribute, then everything else.

7.4 Custom domains appear at the bottom of the list, after the canonical-library domains.

7.5 Clicking a domain button toggles its expanded state.

7.6 The `*` (Universal), `howto`, and `contributing` domains are expanded by default on launch.

7.7 The How To section contains a single entry: "Camerata user guide (read this first)". Clicking it shows the formatted in-app user guide in the right pane. The guide explains the typical workflow, the action bar buttons, the autosave behavior, and the vocabulary.

---

## 8. Expanded domain contents

8.1 Just below the domain button, when expanded, a row of three small action buttons appears: **Defaults**, **All**, **Clear**.

8.2 **Defaults** sets the selection state of this domain's rules back to library defaults (Universal and Choice tags on, Stack tags off).

8.3 **All** selects every rule in this domain.

8.4 **Clear** deselects every rule in this domain.

8.5 Each principle in the domain appears as a row containing:
- A checkbox (except in the meta-documentation domains `howto` and `contributing`, where the checkbox is omitted because those rules are documentation, not selectable conventions)
- A tag glyph in brackets: `[U]` for Universal, `[S]` for Stack, `[C]` for Choice
- The principle title as a clickable blue link

8.6 Clicking the title opens the principle detail in the right column.

8.7 Below the canonical-rule rows, any custom rules attached to this domain appear as grey rows prefixed with "✎ ".

8.8 At the bottom of an expanded domain, a dashed "+ custom rule" button appears — EXCEPT in the meta-documentation domains `howto` and `contributing`, where the button is hidden because those sections are documentation only. (Section 9 explains the canonical-vs-custom distinction.)

---

## 9. Canonical rule vs. custom rule (vocabulary)

9.1 A **canonical rule** is a principle that ships in the camerata-ai library itself, contributed by a domain SME via a pull request to the open-source repo. Canonical rules have a unique traceable id (e.g. `TS-NEXT-CONSENT-GATED-1`).

9.2 A **custom rule** is a rule the user adds to their own session for use in their own project. Custom rules do not require an id, do not need PR review, and live entirely in the user's profile.

9.3 The `contributing` domain in the GUI contains canonical rules that document the schema and contribution process for adding NEW canonical rules. It does not accept custom rules.

---

## 10. "+ custom rule" button and modal

10.1 Visible only on non-`contributing` domains, including user-created custom domains.

10.2 Clicking opens the custom rule modal in the right column. The modal contains:
- A header: "Add a custom rule"
- A grey sub-header: "Domain: <domain label>"
- A text input for the rule name
- A multi-line textarea for the rule body
- An **Add rule** button and a **Cancel** button

10.3 Clicking **Add rule** appends the new custom rule under the chosen domain. The rule appears in the left column under that domain as a "✎ <name>" row.

10.4 Clicking **Cancel** dismisses the modal without saving. The text inputs reset.

10.5 An empty name AND empty body produces no rule (silently dismisses).

---

## 11. "+ Custom domain" button and modal

11.1 At the bottom of the left column (below all domain groups), a full-width dashed button reads "+ Custom domain".

11.2 Clicking opens the custom domain modal in the right column. The modal contains:
- A header: "Add a custom domain"
- A descriptive sentence: "A custom domain holds only your own rules. It will not contain canonical principles from the camerata library."
- A text input for the domain name
- An **Add domain** button and a **Cancel** button

11.3 Clicking **Add domain** with a non-empty trimmed name creates the domain. It appears at the bottom of the left column, expanded by default. The "+ custom rule" button is available there.

11.4 Duplicate domain names are silently ignored (no error, no duplicate row).

11.5 Clicking **Cancel** dismisses the modal without creating a domain.

---

## 12. Principle detail (right column, when a principle row is clicked)

12.1 The detail header shows the principle title.

12.2 Below the title, the principle id appears in small grey monospace font.

12.3 The summary follows, rendered with a markdown-lite formatter:
- Lines starting with `# ` render as section headings (h3-sized).
- Lines starting with `## ` render as sub-headings (h4-sized).
- Consecutive lines starting with `- ` group into a bulleted list.
- Blank lines separate paragraphs.
- Any other non-empty line renders as a paragraph.

Most canonical rules don't use these markers (their summaries are plain prose, which still renders as paragraphs). The Camerata user guide and any future long-form documentation use the markers for readability.

12.4 If the principle has a `why` field, it appears as a paragraph prefixed with bold "Why:".

12.5 For canonical rules NOT in the `contributing` domain, a "Choose how to adopt this:" section follows with one button per option:
- "Adopt as written (default)" — highlighted when selected
- "Alternative: <text>" — one per `alternatives` entry from the rule
- "Your alternative: <text>" — one per user-added alternative on this rule

12.6 Selecting a button updates the chosen option for this rule. The selected button is highlighted blue; unselected buttons have a white background.

12.7 For canonical rules in the meta-documentation domains (`howto` and `contributing`), the "Choose how to adopt" section is hidden because those rules are documentation, not adoptable conventions.

12.8 Below the alternative buttons, a "Add your own alternative (include the context it requires):" section provides:
- A textarea for the alternative description
- An **Add alternative** button

12.9 Adding a non-empty alternative appends it to this rule's user-alternatives list, automatically selects it, and clears the textarea.

---

## 13. Empty-state tip (right column, when nothing else is showing)

13.1 When no principle is selected, no custom rule is being added, and no custom domain is being added, the right column shows a grey tip: "Select a principle on the left, or use the + custom rule button under a domain to add your own."

---

## 14. Autosave behavior

14.1 Autosave fires automatically whenever any tracked state changes: selections, choices, custom alternatives, custom rules, custom domains, output dir, repos, or per-domain routing.

14.2 The first fire on launch is skipped — a fresh launch with no user interaction does NOT create an autosave file. This prevents a misleading recovery banner on the next launch.

14.3 After the first real user mutation, the autosave file is written and is overwritten on every subsequent change.

14.4 Autosave failures (disk full, permission denied) are silently ignored to avoid disrupting the workflow. The user can manually Save Profile for guaranteed durability.

14.5 Autosave lifecycle by event:
- **Generate (successful)**: autosave deleted. Generate is the terminal "commit" action — the in-progress state has been emitted to the target, so there's nothing left to recover.
- **Generate (failed)**: autosave preserved. The work is still in progress.
- **Save and exit** (in exit-confirm banner): autosave deleted. The selection is now in the named profile file.
- **Discard and exit** (in exit-confirm banner): autosave deleted. User explicitly threw it away.
- **Start over** (in recovery banner): autosave deleted.
- **Resume** (in recovery banner): autosave consumed (loaded into state), then the next state change re-creates it.
- **Save Profile…**: autosave preserved. Save Profile is a durable named copy; the in-progress autosave continues mirroring live state until Generate or an explicit exit clears it.
- **Load Profile…**: autosave preserved. The newly-loaded state continues to autosave normally.
- **Export JSON…**: autosave preserved. Export JSON is an interchange snapshot, not a session commit.
- **Cancel** (in Generate-confirm or exit-confirm banner): autosave preserved. The user backed out.
- **Window killed externally** (process kill, crash, power loss): autosave preserved. Recovery banner appears on next launch.

---

## 15. Profile schema (Save Profile / Load Profile JSON)

15.1 The profile JSON contains:
- `version`: integer schema version (currently 1)
- `selected_ids`: array of canonical-rule ids the user selected
- `chosen`: object mapping rule id → chosen alternative text
- `custom_alternatives`: object mapping rule id → array of user-authored alternatives
- `custom_rules`: array of `{ name, body, domain }` objects (full content)
- `custom_domains`: array of user-created domain name strings
- `out_dir`: output directory string
- `repos`: array of target repo path strings
- `domain_repos`: object mapping domain name → array of repo path strings

15.2 On load, canonical-rule content (summary, why, alternatives) is read from the CURRENT library by id. This means library updates to a canonical rule's content are picked up automatically on the next profile load.

15.3 Custom content (rules, domains, alternatives) is restored in full because the user is the source of truth for it.

---

## 16. Generate output

16.1 Clicking **Generate** writes the scaffolded files into the output directory (or per-domain repos when configured).

16.2 Each target file contains the installed rules for its domain set, formatted according to the principle's `enforcement` level (prose / structured / mechanical).

16.3 The default emit target filename is `AGENTS.md` (the cross-tool open standard for AI-agent configuration).

16.4 A success status appears showing the target paths and rule counts. Failures appear as an "Error: …" status.

---

## 17. Known constraints / gotchas

17.1 The autosave path requires platform data-local-dir availability. If `dirs::data_local_dir()` returns None (rare; some embedded or container environments), autosave silently no-ops and recovery is unavailable.

17.2 Profiles do NOT carry forward the GUI's collapse/expand state of domain groups; on load, all newly-loaded custom domains start expanded; library domains use the launch defaults.

17.3 Custom rules and custom domains are scoped to the profile, not to the library. They do not propagate to the camerata-ai open-source repo unless explicitly contributed via a pull request (and at that point they become canonical rules per section 9).

17.4 The recovery banner is mutually exclusive with the missing-ids banner only in time, not in space; if a Resume produces missing ids, both can appear momentarily before Resume dismisses the recovery banner.
