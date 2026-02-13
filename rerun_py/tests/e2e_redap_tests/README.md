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

