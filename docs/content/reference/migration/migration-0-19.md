---
title: Migrating from 0.18 to 0.19
order: 190
---

### C++ and C
You need to link with glibc 2.17 or higher.

### Rust
- Update MSRV to Rust 1.79 [#7563](https://github.com/rerun-io/rerun/pull/7563)
- Update ndarray to 0.16 and ndarray-rand to 0.15 [#7358](https://github.com/rerun-io/rerun/pull/7358) (thanks [@benliepert](https://github.com/benliepert)!)
