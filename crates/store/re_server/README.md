# Rerun server

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_server.svg)](https://crates.io/crates/re_server)
[![Documentation](https://docs.rs/re_server/badge.svg)](https://docs.rs/re_server)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

In-memory opensource implementation of the Rerun server.

The goal for this crate is to support most of the same gRPC endpoints that our commercial Rerun Cloud service supports, but do so in-memory for maximum simplicity.

We use this internally for testing, but in the future it might be useful for users too.

This is (currently) NOT the server you get when running `rerun --serve-grpc`, though we hope to unify the two at some point.
