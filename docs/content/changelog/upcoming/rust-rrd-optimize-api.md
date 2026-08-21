---
title: "Optimize a recording from Rust with `rrd::optimize`"
hidden: true
type: feature
---

### Optimize a recording from Rust with `rrd::optimize`

Add rust support for optimize, behind the `rrd` feature (enabled by default).
Previously, only supported through CLI and python.

`optimize` reads from any reader and writes to any writer, so the output can go straight to a memory buffer, a socket, or an object store upload:

```rust
let profile = rerun::rrd::OptimizationProfile::OBJECT_STORE;

let input = std::io::Cursor::new(std::fs::read("input.rrd")?);
let mut output: Vec<u8> = Vec::new();
let stats = rerun::rrd::optimize(input, &mut output, &profile)?;
println!("wrote {} chunks, {} bytes of messages", stats.num_chunks, stats.num_bytes);
```

`optimize_file` is the shorthand for the file-to-file case.
It reads the input in full before it truncates the output, so both may be the same file:

```rust
rerun::rrd::optimize_file("input.rrd", "output.rrd", &profile)?;
```

Optimize currently holds the whole recording in memory while it works.

This change also corrects the version stamped on the output of `rerun rrd optimize` and `rerun rrd merge` when the input holds more than one store.
They used to take the version of whichever store came first in a hash map, and fall back to the writing version when that store declared none.
They now take the newest version across all the input stores.

Docs: ../howto/logging-and-ingestion/optimize-chunks.md
