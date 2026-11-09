---
title: Python SDK
order: 100
description: Setting up the Python SDK, and what Python versions and platforms it supports
---

The Python SDK includes both the SDK and the Viewer, so you're ready to go with a single install:

-   `pip install rerun-sdk` via pip
-   `conda install -c conda-forge rerun-sdk` via Conda

Conda always comes with support for all features but if using pip you may need to specify optional features:
-   `pip install rerun-sdk[notebook]` for the embedded notebook tools
-   `pip install rerun-sdk[catalog]` for the query api tools
-   `pip install rerun-sdk[dataloader]` for model training tools

## Requirements

-   Python 3.11 or newer.
-   NumPy 2 or newer.
-   One of the platforms that we publish wheels for on PyPI:
    -   Linux on x86_64 or aarch64, with glibc 2.28 or newer (the wheels are `manylinux_2_28`).
        For example, Ubuntu 18.04 is too old.
    -   Windows on x86_64.
    -   macOS on Apple silicon (arm64).
        There are no wheels for Intel Macs.

If `pip` can't find a matching wheel, it reports `No matching distribution found for rerun-sdk`.
See [Troubleshooting](troubleshooting.md#pip-install-issues).

Use the same version of the SDK and the Viewer.
If you also installed the Viewer some other way, make sure the `rerun` on your `PATH` is the one that came with `rerun-sdk`.

### Bazel and other build systems

The wheel installs the `rerun`, `rerun_cli`, and `rerun_bindings` packages directly into `site-packages`.
You don't need to add `site-packages/rerun_sdk` to `PYTHONPATH`.
If you did that to work around an older version of the wheel, remove it.

## Next steps

[Set up a Python project](../../getting-started/project-setup/python.md), then walk through the [Log and Ingest](../../getting-started/data-in.md) tutorial.

-   [Python API docs](https://ref.rerun.io/docs/python)
