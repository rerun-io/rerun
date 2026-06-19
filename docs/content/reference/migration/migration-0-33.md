---
title: Migrating from 0.32 to 0.33
order: 977
---

## `rerun-sdk[dataplatform]` and `rerun-sdk[datafusion]` renamed to `rerun-sdk[catalog]`

The Python optional-dependency extra for catalog/query API tools has been renamed to `catalog`.

| Before                       | After                 |
|------------------------------|-----------------------|
| `pip install rerun-sdk[dataplatform]` | `pip install rerun-sdk[catalog]` |
| `pip install rerun-sdk[datafusion]`   | `pip install rerun-sdk[catalog]` |

The old `dataplatform` and `datafusion` extras still resolve to the same set of dependencies for backwards compatibility, but will be removed in a future release. <!-- NOLINT -->
Update any `pyproject.toml`, `requirements.txt`, or install scripts to the new name.
