---
title: "`rerun dump-puffin` converts puffin profiler recordings to JSON"
hidden: true
type: feature
---

### `rerun dump-puffin`

`rerun dump-puffin recording.puffin > out.json` converts a [puffin](https://github.com/EmbarkStudios/puffin) profiler recording (`.puffin` file) into a single JSON document, so the scope tree can be inspected with tools like `jq`, or by an agent, without compiling any Rust.

See the [CLI manual](../reference/cli.md#rerun-dump-puffin) for details.
