# Rerun server

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_server.svg)](https://crates.io/crates/re_server)
[![Documentation](https://docs.rs/re_server/badge.svg)](https://docs.rs/re_server)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Open-source implementation of the Rerun catalog server.

The goal for this crate is to support most of the same gRPC endpoints that our commercial Rerun Hub service supports.
Catalog metadata is kept in memory.
RRDs with a footer are read from disk on demand.

This crate powers the local `rerun server` command and `rr.server.Server`.

This is (currently) NOT the server you get when running `rerun --serve-grpc`, though we hope to unify the two at some point.
