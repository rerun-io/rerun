---
title: Catalog staging
hidden: true
type: breaking
---

### Catalog staging

Use the experimental [`CatalogClient.stage()`](https://ref.rerun.io/docs/python/stable/common/catalog/#rerun.catalog.CatalogClient.stage) API to upload local files or bytes to catalog storage before registering them with a dataset.
This API may change in future versions.

The legacy `WriteChunks` API has been removed; see the [migration guide](../reference/migration/migration-0-39.md#writechunks-removed).
