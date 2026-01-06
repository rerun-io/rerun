---
title: Migrating from 0.28 to 0.29
order: 981
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Deprecated `rerun.dataframe` API has been removed

The `rerun.dataframe` module and its associated APIs, which were deprecated in 0.28, have now been fully removed. This includes `RecordingView`, `Recording.view()`, and the ability to run dataframe queries locally via this module.

Please refer to the [0.28 migration guide section on `RecordingView` and local dataframe API](migration-0-28.md#recordingview-and-local-dataframe-api-deprecated) for details on updating your code to use `rerun.server.Server` and the `rerun.catalog` API instead.
