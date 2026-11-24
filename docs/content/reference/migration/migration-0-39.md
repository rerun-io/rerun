---
title: Migrating from 0.38 to 0.39
order: 975
hidden: true
description: Breaking changes in Rerun 0.39 and how to update code written for 0.38
---

## WriteChunks removed

The legacy `WriteChunks` API has been removed.
To add a recording, stage an RRD with [`CatalogClient.stage()`](https://ref.rerun.io/docs/python/stable/common/catalog/#rerun.catalog.CatalogClient.stage), then register the returned URL.
