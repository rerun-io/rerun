---
title: Migrating from 0.25 to 0.26
order: 984
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Python SDK: removed `blocking` argument for `flush`
Use the new `timeout_sec` argument instead.
For non-blocking, use `timeout_sec=0`.
Mostly you can just call `.flush()` with no arguments.
That will block until all writes either finishes or an error occurs (e.g. the gRPC connection is severed).

## Python SDK: more use of kw-args
We have started using named arguments (kw-args) for more of our functions.
This will make it easier for us to evolve our APIs in the future, when adding new arguments, or renaming old ones.

Before:
```py
rr.ImageFormat(width, height, "YUV420")

blueprint.spawn("my_app", 1234)
```

After:
```py
rr.ImageFormat(width, height, pixel_format="YUV420")

blueprint.spawn("my_app", port=1234)
```

[ruff](https://github.com/astral-sh/ruff) (or your preferred Python linter) will find all the places in your code that need to be updated!

## Python DataFusion interface: update to 49.0.0
The DataFusion FFI that we rely on for user defined functions and
table providers requires users to upgrade their `datafusion-python`
version to 49.0.0. This only impacts customers who use the
DataFusion tables provided through the `CatalogClient`.
