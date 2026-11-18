---
title: Dataloader curations replace sampling manifests
hidden: true
type: breaking
---

Replace the experimental `Manifest` / `from_manifest()` API with [`Curation.from_source()` and `DataSource(dataset, curation=curation)`](../train/dataloader.md#train-on-selected-spans); v1 manifest files must be regenerated as curations from selected recordings and optional span intervals.
The resolved Arrow output is the training manifest; content filters, fields, and sampling remain loader configuration, `Field.max_staleness` is removed, and iterable loaders skip unresolved samples instead of enforcing strict frozen replay.
Bulk `SpanTable.write()` with `mode="append"` or `"overwrite"` now generates IDs and rejects supplied non-null IDs; use `mode="upsert"` with nonempty, non-null IDs to preserve them.
