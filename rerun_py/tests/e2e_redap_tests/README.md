# End-to-end redap tests

End-to-end test suite for redap (Rerun Data Protocol).

## Overview

This test suite exercises the full redap stack by using the Python SDK (primarily `CatalogClient`) against a live Rerun server.

## Architectural notes

The `catalog_client` fixture is the foundation of this test suite. It yields a connected `CatalogClient` instance which all other fixtures and tests depend on.

By default, the fixture creates a local OSS server for each test. However, it can be configured to connect to an external redap server using the `--redap-url` option, allowing the test suite to run against different redap implementations (e.g., Cloud deployments).

## Running tests

Note: prefix commands with `pixi run uvpy -m` to run in the pixi/uv environment.

Run against a local OSS server (default):
```bash
pixi run uvpy -m pytest -c rerun_py/pyproject.toml rerun_py/tests/e2e_redap_tests
```

Run against an external redap server:
```bash
pytest -c rerun_py/pyproject.toml rerun_py/tests/e2e_redap_tests --redap-url=rerun+http://localhost:51234
```

With authentication:
```bash
pytest -c rerun_py/pyproject.toml rerun_py/tests/e2e_redap_tests --redap-url=rerun+https://example.com --redap-token=your_token
```

Skip local-only tests (useful for Docker/containerized environments):
```bash
pytest -c rerun_py/pyproject.toml rerun_py/tests/e2e_redap_tests -m "not local_only"
```

Note: When using `--resource-prefix` with remote storage (s3://, gs://, etc.), local-only tests are automatically skipped.

## CI

In Rerun's internal CI, this suite runs in several OSS↔Hub compatibility configurations:

| Job                        | Server                                  | SDK + tests            | Profile      | Status   |
| -------------------------- | --------------------------------------- | ---------------------- | ------------ | -------- |
| `e2e-tests`                | Current hub (Docker)                    | Current commit         | `dpf-docker` | Required |
| `e2e-tests-oldest-hub`     | Oldest supported hub release (Docker)   | Current commit         | `dpf-docker` | Advisory |
| `e2e-tests-released-client`| Current hub (Docker)                    | Latest hub release     | `dpf-docker` | Advisory |
| cloud stacks               | Current hub (AWS/Azure stack)           | Current commit         | `dpf-stack`  | Manual dispatch only |

The oldest-supported hub version and per-leg test deselections are pinned in a compat file next to the hub sources (internal, `compat/e2e-compat.json`).

Tests marked `@pytest.mark.local_only` are skipped in both CI profiles (they require writing local `.rrd` files).
Tests marked `@pytest.mark.cloud_only` only run against cloud stacks (`dpf-stack`).

## Version compatibility gating

The `e2e-tests-oldest-hub` CI leg runs the *current* tests against an *old* hub server, so tests exercising newer server behavior must declare their requirements:

- `@pytest.mark.requires_server_feature("some_feature")` — preferred.
  Skips unless the server advertises the feature in its `Version` RPC response (see the `features` module in `re_protos` for the canonical feature name constants).
  Old servers that predate feature advertising return an empty list and are skipped automatically.
- `@pytest.mark.min_hub_version("0.16.0")` — for behavior changes without a feature flag.
  Only enforced when the target is a hub (any profile except `local`), because the OSS server is versioned on the rerun scheme, not the hub scheme.
  Pre-release suffixes are ignored when comparing, so an in-tree `0.16.0-alpha.1` hub satisfies `0.16.0`.

**Convention:** when a test of brand-new server behavior fails on the oldest-hub leg, that red is the signal to add a one-line marker (or, for pure snapshot drift with no feature to gate on, an entry in `oldest_hub_deselects` in the compat file mentioned above).
The leg is advisory, so an unmarked new test blocks nobody while it's being triaged.

## Related test suites

There are more e2e tests in [`re_redap_tests`](../../../crates/store/re_redap_tests/README.md), written in Rust.
