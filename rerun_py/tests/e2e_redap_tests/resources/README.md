# Resources for the E2E redap tests

Test resources for the `e2e_redap_tests` suite, stored in Git LFS.

## Contents

### `dataset/`

Collection of 20 .rrd files for testing dataset registration, querying, and partitioning.

See [README.md](dataset/README.md) for more details.


### `simple_datatypes/`
Lance table containing sample data with basic datatypes (int, bool, float).

- Format: Lance table (_transactions, _versions, data directories)
- Used by: `readonly_table_uri` fixture, table read/write tests
- Used for testing DataFusion operations and table registration


## Remote resources

When running tests against remote deployments, use `--resource-prefix` to point to S3/GCS copies of these resources:

```bash
pytest … --resource-prefix=s3://bucket/path/to/resources/
```

The prefix should point to a directory containing `dataset/` and `simple_datatypes/` subdirectories.
