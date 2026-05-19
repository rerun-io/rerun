# Design guidelines
Guidelines for UI design, covering GUI, CLI, documentation, log messages, etc

## Text
#### Sentences
Short, single-sentence text do NOT end in a period in the UI.

Multi-sentence text ALWAYS ends in a period.

#### Casing
We prefer normal casing, and avoid Title Casing.

We only use Title Casing for company and product names (Rerun, Rerun Viewer, Discord, …), but NOT for concepts like “container”, “view”, etc.

#### Examples
Good: `log("File saved")`

Bad: `log("file saved.")`

#### Dashes
Use a spaced em dash (` — `) for parenthetical breaks in prose (docs, comments, log messages, UI text).

Avoid:
- Unspaced em dashes (`word—word`) — add spaces around the em dash. <!-- NOLINT -->
- En dashes (`–`) used as sentence punctuation — use an em dash instead.

En dashes are reserved for numeric/range expressions (`2020–2025`, `pp. 10–15`, `~3–4 GB`).

#### Line breaks in markdown
Write one sentence per line in markdown files (`.md`, docs, READMEs, agent guides).
Markdown joins consecutive non-empty lines into a single paragraph, so this does not affect rendering — but it produces much cleaner diffs.
Each edited sentence shows up as a single changed line, instead of reflowing an entire paragraph.

Use a blank line between paragraphs as usual.

### Buttons

When a button action requires more input after pressing, suffix it with `…`.

Good: `Save recording…` (leads to a save-dialog)

## GUI labels

We do not use a colon suffix for labels in front of a value.

Good: `Color 🔴`

Bad: `Color: 🔴`
