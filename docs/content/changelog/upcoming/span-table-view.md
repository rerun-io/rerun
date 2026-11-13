---
title: Query annotated time spans
hidden: true
type: feature
---

The experimental `rerun.experimental.spans.SpanTable` stores time span annotations, such as task labels and scores, for a dataset's recordings in a catalog table.
Add and edit spans with `append` and `upsert`, then select them with DataFusion filters on `reader()`, joined with the recordings' properties.
Only duration and timestamp timelines are supported, not sequence timelines.
See [Curating annotated time spans](../query-and-transform/dataframes/spans.md) for a walkthrough.
