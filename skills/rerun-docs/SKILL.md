---
name: rerun-docs
description: Where the Rerun documentation and code snippets live and how to search them. Read this before answering a question about how Rerun works, what an archetype or view does, how to log or query data, or when you need a copy-pastable example in Python, Rust, or C++.
user_invocable: true
allowed-tools: Read, Grep, Glob, WebFetch
---

# Rerun docs

The source of <https://rerun.io/docs> and every code snippet it shows are plain files. Read them instead of guessing.

## Where

- In a Rerun checkout: `docs/`.
- Anywhere else: fetch the files for the Rerun version in use from GitHub, e.g. <https://raw.githubusercontent.com/rerun-io/rerun/0.27.0/docs/snippets/INDEX.md> for 0.27.0.
  Get the version from `rerun --version`, `rerun.__version__`, or the Viewer you are connected to.
  Use `main` only for a development build: it may describe features that are not released yet.
  <https://rerun.io/docs> shows only the latest release, which can be newer than the version in use.

## Layout

| Path | What |
|------|------|
| `docs/content/**/*.md` | The website pages. `docs/content/viewer/<section>/<name>.md` maps to `rerun.io/docs/viewer/<section>/<name>`. |
| `docs/content/reference/types/` | One page per archetype, component, view, and blueprint type. |
| `docs/content/reference/cli.md` | CLI reference. |
| `docs/content/getting-started/` | Install, project setup, and how Rerun works. |
| `docs/content/agents.md` | Using Rerun with coding agents: the skills, the `mcp` server, and the Viewer's agent panel. |
| `docs/content/log/`, `viewer/`, `catalog/`, `ingest/`, `query-and-transform/`, `train/` | One folder per product area, some with a `tutorial.md`, and a few topic groups. The Viewer UI and `mcp` server docs are under `viewer/`; SDK operating modes and batching are under `log/sinks-and-modes/`; Rerun Hub is `catalog/hub.md`. Integrations are under `viewer/integrations/` and `log/integrations/`. |
| `docs/content/changelog/` | Release notes, oldest to newest. |
| `docs/snippets/INDEX.md` | Table of every snippet: feature, name, one-line description, and which languages exist. Start here. |
| `docs/snippets/all/<category>/<name>.{py,rs,cpp}` | The snippets themselves. `archetypes/`, `views/`, `howto/`, `concepts/`, `tutorials/`, `quick_start/`. Same name in each language means the same example. |

Each markdown page starts with front matter (`title`, `order`); the body is the page.

## How

1. Search first, then read: `grep -rl "<term>" docs/content docs/snippets/all` in a checkout, or start from `docs/snippets/INDEX.md` online.
2. For "how do I log X": open `docs/snippets/all/archetypes/<x>.py` (or `.rs`, `.cpp`) and `docs/content/reference/types/archetypes/<x>.md`.
3. For "how does X work": the matching product-area folder, or `docs/content/getting-started/how-does-rerun-work.md` for the big picture.
4. When pointing the user at a page, give the website URL derived from the file path, not the local path.
5. The Python API reference (`https://ref.rerun.io/docs/python/<version>/`) and `docs.rs/rerun/<version>` are not on disk; fetch them for the version in use.
