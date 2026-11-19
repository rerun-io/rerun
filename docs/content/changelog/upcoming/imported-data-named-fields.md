---
title: "`ImportedData` and `SplitFrom` have named fields"
hidden: true
type: breaking
---

### `ImportedData` and `SplitFrom` have named fields

The variants of the Rust `ImportedData` enum, which custom importers send, now have named fields instead of positional ones.
`Chunk` and `ArrowMsg` have `importer_name`, `store_id`, and `chunk` or `msg`.
`LogMsg` has `importer_name` and `msg`.

The `SplitFrom` variants of `ChunkDirectLineage` and `ChunkDirectLineageReport` (see `ChunkStoreDiffAddition::direct_lineage`) also have named fields now.

#### Migration

Rust, before:

```rust
let data = ImportedData::Chunk(importer_name, store_id, chunk);
```

Rust, after:

```rust
let data = ImportedData::Chunk {
    importer_name,
    store_id,
    chunk,
};
```

Matches change the same way, e.g. `ImportedData::LogMsg(_, msg)` becomes `ImportedData::LogMsg { msg, .. }`.
`ChunkDirectLineage::SplitFrom(parent_id, sibling_ids)` becomes `ChunkDirectLineage::SplitFrom { parent_id, sibling_ids }`.
`ChunkDirectLineageReport::SplitFrom(parent, siblings)` becomes `ChunkDirectLineageReport::SplitFrom { parent, siblings }`.
