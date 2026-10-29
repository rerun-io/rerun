# re_catalog_client

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_catalog_client.svg)](https://crates.io/crates/re_catalog_client)
[![Documentation](https://docs.rs/re_catalog_client/badge.svg)](https://docs.rs/re_catalog_client)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

High-level Rust APIs for Rerun catalogs, shared by the SDK and viewer and backed by `re_redap_client`.

Consider both the Python SDK (`rerun_py`) and the viewer when adding features so the API serves both well.
Keep this crate focused on generally applicable catalog operations rather than single-use helpers.

Downstream crates are encouraged to extend `CatalogClient` with their own extension traits.
As common needs emerge, those features can be generalized into `re_catalog_client`.

Give this crate the same API design and review scrutiny as the crates in `crates/top`, since it may move there in the future.
