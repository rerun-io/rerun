---
title: Migrating from 0.18 to 0.19
order: 991
---

Blueprint files (.rbl) from previous Rerun versions will no longer load _automatically_.

### 🐧 Linux
Rerun now require glibc 2.17 or higher.

This is because we updated the Rust version. See <https://blog.rust-lang.org/2022/08/01/Increasing-glibc-kernel-requirements.html> for more details.

### 🦀 Rust
- Update MSRV to Rust 1.79 [#7563](https://github.com/rerun-io/rerun/pull/7563)
- Update ndarray to 0.16 and ndarray-rand to 0.15 [#7358](https://github.com/rerun-io/rerun/pull/7358) (thanks [@benliepert](https://github.com/benliepert)!)
