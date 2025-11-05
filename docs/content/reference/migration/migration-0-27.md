---
title: Migrating from 0.26 to 0.27
order: 983
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Dropped support for Intel Macs
We've dropped official support for Intel (x86) maxOS in [PR #11719](https://github.com/rerun-io/rerun/pull/11719).

This means our Python pheels on PyPi.org and our other pre-built artifact does no longer include Intel Mac binaries.

You can still build Rerun from source.
There are instructions for that in [`BUILD.md`](https://github.com/rerun-io/rerun/blob/main/BUILD.md).


## Python SDK: minimum supported Python 3.10

Support for Python 3.9 is past end-of-life.
See https://docs.python.org/3/whatsnew/3.10.html for more details on upgrading to 3.10 if necessary.

