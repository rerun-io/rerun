---
title: "`object_store_config` renamed to `storage_location`"
hidden: true
type: breaking
---

In `DatasetEntry.register()`, replace `object_store_config` with `storage_location`; the old keyword still works with a deprecation warning.
Supplying both keywords raises `ValueError`, even if their values match.
Invalid storage location aliases now raise `ValueError`.
