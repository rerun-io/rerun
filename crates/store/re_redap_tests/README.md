# re_redap_tests

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.


![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Official test suite for the Rerun Data Protocol ("redap").

This test suite is specifically focused on the redap layer.
In particular it aims to cover what our API's `*.proto` files leave implicit.
This includes at least:
- all the dataframes (schema, content)
- all the stateful behaviors (e.g. chunk keys, tasks, etc.)

As such, it is implemented to be as close as possible to the actual API boundary, aka the (incorrectly named) `RerunCloudService` trait.

## Goals

- Cover all aspects of the redap layer, including dataframe schemas and stateful behaviors.
- Serve as the definitive reference of what redap is.
- Ensure conformance of all implementations (including, possibly, third-party).

## Non-goals

- Test layers outside the redap boundary, including `re_redap_client::ConnectionClient` or the Python SDK.
- Test anything about the internals of the redap implementors (OSS server, Rerun Cloud, etc.)

## Usage

This crate provides the test suite, but it requires an actual implementation
of the server in order to run these tests. To use the OSS rerun server to
perform these tests use the following command

```shell
cargo test -p re_server --all-features
```
