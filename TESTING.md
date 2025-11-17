# Testing

This is an overview of our testing infrastructure.

## See also
* [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)



## Rust unit tests

We use the standard Rust test framework.
We like to use [nextest](https://nexte.st) because it's faster, but `cargo test` should work fine.

While developing, default to using `--all-features`.
(Our CI runs checks and tests with many feature configurations.)

Typically:

```
cargo nextest run -p re_XXX --all-features
```
where `re_XXX` is the crate under development.

### Snapshot tests

We use [insta](https://insta.rs) for snapshot tests.

To create or update snapshots, you may use:

```
INSTA_FORCE_PASS=1 cargo nextest run -p re_XXX --all-features
```

Then, use this to review the created/updated snapshots:

```
cargo insta review
```

### UI snapshot tests

We use [`egui_kittest`](https://github.com/emilk/egui/tree/main/crates/egui_kittest) extensively for UI snapshot tests.

To create or update snapshots, you may use:

```
UPDATE_SNAPSHOTS=1 cargo nextest run -p re_XXX --all-features
```

Our CI automatically provides a link in PR for [kitdiff](https://github.com/rerun-io/kitdiff), our custom visual image diff tool.


## Python unit tests

We use [pytest](https://docs.pytest.org/) for Python unit tests, along with [syrupy](https://github.com/syrupy-project/syrupy) for snapshot testing.

To run tests, use:

```
pixi run py-test
```

Creating or updating snapshots is done by adding `--snapshot-update` to the pytest command.


## Redap tests

Redap stands for "Rerun data protocol." It is the interface between clients such as the Rerun viewer or SDK, and servers such as Rerun OSS or Rerun Cloud.

We have several test harnesses related to redap.

### `re_redap_tests`

This is a Rust-based compliance test suite that builds directly against the server's service handler. It is run both against the OSS server in this repository, and our Rerun's proprietary implementation Rerun Cloud. This test suite does not run through an actual gRPC connection. It directly links to the servers' code.

This test suite is executed by the OSS server tests, so you can run it locally with:

```
cargo nextest run -p re_server --all-features
```

### `re_integration_test`

This is a Rust-based test suite that runs both our Rust-based client and the OSS server. The harness spins up a local server and the tests connect to it.

Run it with:

```
cargo nextest run -p re_integration_test --all-features
```


### `e2e_redap_tests`

This is a python-based end-to-end test suite that uses our Python SDK and connects, by default, to the OSS server. It is executed by `pytest`. More in formation in the [`e2e_redap_tests` README](rerun_py/tests/e2e_redap_tests/README.md).


