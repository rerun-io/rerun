# re_build_tools

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_build_tools.svg)](https://crates.io/crates/re_build_tools)
[![Documentation](https://docs.rs/re_build_tools/badge.svg)](https://docs.rs/re_build_tools)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Library to be used in `build.rs` files in order to build the build info defined in `re_build_info` by setting environment variables.

Some information in `re_build_info` can be provided through env vars:

- `GIT_HASH` (the full sha, like `e264b9decab9257ae79100a006bb69c0d289e20c`)
- `GIT_BRANCH` (the `symbolic-ref --short`, like `asdf/my-branch`)
- `DATETIME` (ISO8601, like `2025-12-10T15:49:52.089915278Z`)

If these are required but not provided, they will be gathered by:

- `GIT_HASH`: running `git rev-parse HEAD`
- `GIT_BRANCH`: running `git symbolic-ref --short HEAD`
- `DATETIME` retrieving system time during the build

If you'd like to avoid these for whatever reason, then set them externally.
