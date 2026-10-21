---
title: New streaming dataset-query pipeline enabled by default
hidden: true
type: feature
---

### New streaming dataset-query pipeline enabled by default

Dataset queries now stream through a redesigned client pipeline.
Fetches are planned up front and issued in emission order, memory is bounded by a single byte window instead of a mix of gates and budgets, and cancellation reliably aborts in-flight downloads.
Behavior changes to be aware of:

* Rows are never emitted before a segment's static chunks have arrived, so results on datasets with late-arriving static data are now complete where they could previously miss static values.
* The `RERUN_PIPELINE_BUDGET_*` and `RERUN_SEGMENT_ADMISSION_*` environment variables no longer affect the default path.
  The new knobs are `RERUN_DATAFRAME_WINDOW_BYTES` and `RERUN_DATAFRAME_FETCH_CONCURRENCY`.

Set `RERUN_DATAFRAME_PIPELINE=v1` to fall back to the previous pipeline.
The fallback will be removed in the next release.
