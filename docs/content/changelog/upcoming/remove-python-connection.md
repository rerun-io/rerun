---
title: Removed deprecated Python connection and event aliases
hidden: true
type: breaking
---

Rerun 0.44 removes the connection and event aliases deprecated in 0.29.

Use `CatalogClient(url=…)` and `Server(host=…)` instead of `addr=`.
Replace `server.address()` with `server.url()` and `event.partition_id` with `event.segment_id`.
