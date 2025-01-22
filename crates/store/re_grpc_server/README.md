# re_grpc_server

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_grpc_server.svg?speculative-link)](https://crates.io/crates/re_grpc_server?speculative-link)
[![Documentation](https://docs.rs/re_grpc_server/badge.svg?speculative-link)](https://docs.rs/re_grpc_server?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Server implementation of an in-memory Storage Node.

## Usage

To run the server locally:

1. Install [Envoy](https://www.envoyproxy.io/docs/envoy/latest/start/install)
2. `cargo run -p re_grpc_server --release`
3. `envoy -c envoy.yml`

The server is available on `127.0.0.1:1853`.
