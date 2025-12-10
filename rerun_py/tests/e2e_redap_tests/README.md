# End-to-end redap tests

End-to-end test suite for redap (Rerun Data Protocol).

## Overview

This test suite exercises the full redap stack by using the Python SDK (primarily `CatalogClient`) against a live Rerun server.

## Architectural notes

The `catalog_client` fixture is the foundation of this test suite. It yields a connected `CatalogClient` instance which all other fixtures and tests depend on.

By default, the fixture creates a local OSS server for each test. However, it can be configured to connect to an external redap server using the `--redap-url` option, allowing the test suite to run against different redap implementations (e.g., Cloud deployments).

## Running tests

Note: prefix everything by `pixi run -e py` to run in a the pixi environment.

Run against a local OSS server (default):
```bash
pytest -c rerun_py/pyproject.toml rerun_py/tests/e2e_redap_tests
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


## Inventory of existing markers

- `benchmark`: Marks performance benchmark tests. These tests may take longer to run and are typically used for performance regression testing.
- `cloud_only`: Marks tests that should only run against remote redap stacks (e.g., Cloud deployments). These tests may rely on features not available in local OSS servers.
- `creates_table`: Marks tests as creating a table (which requires providing a server-accessible path). This is semantically equivalent to `local_only` , but it reminds us that we need to clean this up (RR-2969)
- `local_only`: Marks tests that should only run against a local OSS server. These tests may rely on features not available in remote deployments (e.g., direct filesystem access).
- `slow`: Marks tests that are slow to run and you may want to skip in a tight dev loop",
