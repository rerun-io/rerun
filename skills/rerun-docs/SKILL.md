---
name: rerun-docs
description: Where the Rerun documentation and code snippets live on disk and how to search them. Read this before answering a question about how Rerun works, what an archetype or view does, how to log or query data, or when you need a copy-pastable example in Python, Rust, or C++.
user_invocable: true
allowed-tools: Read, Grep, Glob, WebFetch
---

# Rerun docs

The source of <https://rerun.io/docs> and every code snippet it shows are plain files. Read them instead of guessing.

## Where

- In a Rerun checkout: `docs/`.
- Inside the Rerun Viewer's agent panel: unpacked next to the skills, at `../../../docs` relative to this file. The session preamble names the absolute path.

## Layout

| Path | What |
|------|------|
| `docs/content/**/*.md` | The website pages. `docs/content/concepts/<name>.md` maps to `rerun.io/docs/concepts/<name>`. |
| `docs/content/reference/types/` | One page per archetype, component, view, and blueprint type. |
| `docs/content/reference/viewer/`, `reference/sdk/`, `reference/cli.md` | Viewer, SDK, and CLI reference, including the `viewer-mcp` server. |
| `docs/content/howto/`, `docs/content/getting-started/` | Task-oriented guides and tutorials. |
| `docs/content/changelog/` | Release notes, oldest to newest. |
| `docs/snippets/INDEX.md` | Table of every snippet: feature, name, one-line description, and which languages exist. Start here. |
| `docs/snippets/all/<category>/<name>.{py,rs,cpp}` | The snippets themselves. `archetypes/`, `views/`, `howto/`, `concepts/`, `tutorials/`, `quick_start/`. Same name in each language means the same example. |

Each markdown page starts with front matter (`title`, `order`); the body is the page.
Recording files (`*.rrd`) next to the snippets are not shipped with the viewer.

## How

1. Search first, then read: `rg -l "<term>" docs/content docs/snippets/all`.
2. For "how do I log X": open `docs/snippets/all/archetypes/<x>.py` (or `.rs`, `.cpp`) and `docs/content/reference/types/archetypes/<x>.md`.
3. For "how does X work": `docs/content/concepts/`.
4. When pointing the user at a page, give the website URL derived from the file path, not the local path.
5. The [Python API reference](https://ref.rerun.io/docs/python) and `docs.rs/rerun` are not on disk; use `WebFetch` for those.
