---
title: Migrating from 0.19 to 0.20
order: 990
---


## ⚠️ Breaking changes

### `re_query::Caches` -> `re_query::QueryCache` & `re_query::CacheKey` -> `re_query::QueryCacheKey`

`re_query::Caches` has been renamed `re_query::QueryCache`, and similarly for `re_query::CacheKey`.

Note that this doesn't affect `re_dataframe`, where this type was already re-exported as `QueryCache`.

### Python `colors` change in behavior for single-dimensional lists

Single-dimensional lists that don't otherwise provide type information are now be assumed to be packed
integers color representations (e.g. `0xRRGGBBAA`), unless the length is exactly 3 or 4.

In the case of single lists of 3 or 4 elements, we continue to allow the common pattern of writing: `colors=[r, g, b]`.

This change primarily impacts a previous feature in which all lists divisible by 4 were assumed to be alternating,
`[r, g, b, a, r, g, b, a, …]`. This feature is still available, but depends on your input explicitly being typed
as a numpy array of `np.uint8`.

If you depend on code that uses a bare python list of alternating colors, such as:
```python
rr.log("my_points", rr.Points3D(…, colors=[r, g, b, a, r, g, b, a, …]))
```
You should wrap your input explicitly in a `np.uint8` typed numpy array:
```python
rr.log("my_points", rr.Points3D(…, colors=np.array([r, g, b, a, r, g, b, a, …], dtype=np.uint8)))
```

Additionally, if you are making use of packed integer colors, it is also advised to add the `np.uint32` type,
as otherwise length-3 or length-4 lists will risk being interpreted incorrectly.
```python
rr.log("my_points", rr.Points3D(…, colors=[0xff0000ff, 0x00ff00ff, 0x0000ffff, …]))
```
becomes
```python
rr.log("my_points", rr.Points3D(…, colors=np.array([0xff0000ff, 0x00ff00ff, 0x0000ffff, …], dtype=np.uint32)))
```

## ❗ Deprecations

### Python 3.8

Support for Python 3.8 is being deprecated. Python 3.8 is past end-of-life. See: https://devguide.python.org/versions/
In the next release, we will fully drop support and switch to Python 3.9 as the minimum supported version.

### `connect` -> `connect_tcp` & `serve` -> `serve_web`

In all SDKs:
* `connect()` is now deprecated in favor `connect_tcp()`
* `serve()` is now deprecated in favor `serve_web()`

The old methods will be removed in a future release.

The rationale behind this change is that it was easy to confuse what these functions do exactly:

We frequently had reports from users that were understandably expecting a serving process (`rr.serve()`) to be ready to accept connections from other processes (`rr.connect()`), when in reality the two things are completely unrelated: one is hosting a websocket server to be polled by the web-viewer, while the other is trying to connect to the TCP SDK comms pipeline.

You can learn more about Rerun's application model and the different servers and ports by reading our [new documentation page on the matter](../../concepts/how-does-rerun-work.md).
