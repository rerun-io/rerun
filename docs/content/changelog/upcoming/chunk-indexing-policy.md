---
title: Chunk indexing policy in the Python catalog API
hidden: true
type: feature
---

### Chunk indexing policy in the Python catalog API

A dataset can now index only some of its entity paths and timelines, so registration and queries skip the data you do not need.
Pass a `rerun.catalog.ChunkIndexingPolicy` to `CatalogClient.create_dataset(chunk_indexing_policy=…)`, read it back with `DatasetEntry.chunk_indexing_policy`, and change it with `DatasetEntry.set_chunk_indexing_policy`.
Changing it re-indexes the registered data in the background; call `wait()` on the returned `rerun.catalog.DatasetPolicyHandle` to block until that finishes.
Each dimension is a list of names, or `"all"` for every name, including names logged later.
Resolve entity path rules such as `/world/**` against the dataset schema with `ChunkIndexingPolicy.match` before sending them.
Without a policy, a dataset indexes everything, and `DatasetEntry.set_chunk_indexing_policy(None)` resets it to that.

Only Rerun Hub applies the policy; the OSS server stores it and ignores it.
