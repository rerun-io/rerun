---
title: Migrating from 0.28 to 0.29
order: 981
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Deprecated `rerun.dataframe` API has been removed

The `rerun.dataframe` module and its associated APIs, which were deprecated in 0.28, have now been fully removed. This includes `RecordingView`, `Recording.view()`, and the ability to run dataframe queries locally via this module.

Please refer to the [0.28 migration guide section on `RecordingView` and local dataframe API](migration-0-28.md#recordingview-and-local-dataframe-api-deprecated) for details on updating your code to use `rerun.server.Server` and the `rerun.catalog` API instead.

## Deprecated `rerun.catalog` APIs have been removed

The deprecated `rerun.catalog` APIs that were marked for removal in 0.28 have now been fully removed. If you were using any of these deprecated methods, you must update your code to use the new APIs.

Please refer to the [0.28 migration guide section on catalog API overhaul](migration-0-28.md#python-sdk-catalog-api-overhaul) for more details on the new API patterns.

## `Entry.update()` is deprecated in favor of `Entry.set_name()`

The `Entry.update()` method has been deprecated. Use `Entry.set_name()` instead for renaming entries.

```python
# Before (deprecated)
entry.update(name="new_name")

# After
entry.set_name("new_name")
```

The deprecated method will emit a `DeprecationWarning` and will be removed in a future release.
