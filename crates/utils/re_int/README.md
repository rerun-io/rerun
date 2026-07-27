# re_int

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_int.svg)](https://crates.io/crates/re_int)
[![Documentation](https://docs.rs/re_int/badge.svg)](https://docs.rs/re_int?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Small numeric helper traits shared across the Rerun crates.

* `SaturatingCast` — cast between integer types, clamping to the target's range instead of wrapping (`as`) or panicking (`TryFrom` + `unwrap`).
* `UnsignedAbs` — compute the absolute value of a signed integer without wrapping or panicking.
