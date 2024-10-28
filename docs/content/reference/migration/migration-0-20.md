---
title: Migrating from 0.19 to 0.20
order: 990
---


## ⚠️ Breaking changes


### `connect` -> `connect_tcp` & `serve` -> `serve_web`

In all SDKs:
* `connect()` is now deprecated in favor `connect_tcp()`
* `serve()` is now deprecated in favor `serve_web()`

The rationale behind this change is that it was common for users (see https://github.com/rerun-io/rerun/issues/7766).

We frequently had reports from users that were understandably expecting a serving process (`rr.serve()`) to be ready to accept connections from other processes (`rr.connect()`), when in reality the two things are completely unrelated: one is hosting a websocket server to be polled by the web-viewer, while the other is trying to connect to the TCP SDK comms pipeline.

You can learn more about Rerun's application model and the different servers and ports by reading our [new documentation page on the matter](../../concepts/app-model.md).


### `re_query::Caches` -> `re_query::QueryCache` & `re_query::CacheKey` -> `re_query::QueryCacheKey`

`re_query::Caches` has been renamed `re_query::QueryCache`, and similarly for `re_query::CacheKey`.

Note that this doesn't affect `re_dataframe`, where this type was already re-exported as `QueryCache`.
