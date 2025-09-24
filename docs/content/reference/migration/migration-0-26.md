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
