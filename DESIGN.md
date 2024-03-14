# Design guidelines
Guidelines for UI design, covering GUI, CLI, documentation, log messages, etc

## Text
#### Sentences
Short, single-sentence text do NOT end in a period.

Multi-sentece text ALWAYS ends in a period.

#### Casing
We prefer normal casing, and avoid Title Casing.

We only use Title Casing for company and product names (Rerun, Rerun Viewer, Discord, …), but NOT for concepts like “container”, “space view”, etc.

#### Examples
Good: `log("File saved")`

Bad: `log("file saved.")`

### Buttons

When a button action requires more input after pressing, suffix it with `…`.

Good: `Save recording…` (leads to a save-dialog)

## GUI labels

We do not use a colon suffix for labels in front of a value.

Good: `Color 🔴`

Bad: `Color: 🔴`
