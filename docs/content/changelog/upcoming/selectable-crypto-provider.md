---
type: feature
---

### Choose a native cryptographic provider

Rust SDK and viewer consumers can select AWS-LC instead of the default Ring provider for TLS and OAuth token verification.
See the [configuration example and compatibility notes](https://github.com/rerun-io/rerun/blob/main/crates/top/rerun/README.md#native-cryptographic-provider).
Python source builds can also select AWS-LC when the optional Lance support, whose dependencies still select Ring, is disabled.
