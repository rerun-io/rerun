---
title: "`rerun-sdk[datafusion]` and `rerun-sdk[dataplatform]` extras removed"
hidden: true
type: breaking
---

### `rerun-sdk[datafusion]` and `rerun-sdk[dataplatform]` extras removed

Both extras were deprecated in 0.33 in favor of `rerun-sdk[catalog]`, and are now gone.
`pip install` fails on an unknown extra, so update any `pyproject.toml`, `requirements.txt`, `uv` dependency group, or install script that still names them.

| Before                                  | After                            |
|-----------------------------------------|----------------------------------|
| `pip install rerun-sdk[datafusion]`     | `pip install rerun-sdk[catalog]` |
| `pip install rerun-sdk[dataplatform]`   | `pip install rerun-sdk[catalog]` |

The dependency set is unchanged — `catalog` installs the same `datafusion` and `pandas` versions the old extras did.

Docs: ../getting-started/install-rerun/python.md
