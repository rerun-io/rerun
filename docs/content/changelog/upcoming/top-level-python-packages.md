---
title: rerun-sdk installs rerun as a top-level package
hidden: true
type: breaking
---

### `rerun-sdk` installs `rerun` as a top-level package

The `rerun-sdk` wheel now installs `rerun`, `rerun_cli`, and `rerun_bindings` directly into `site-packages`, instead of under `site-packages/rerun_sdk/` with a `rerun_sdk.pth` file.
This means `import rerun` works in a running notebook kernel right after `%pip install rerun-sdk`, without a restart.

If you added `site-packages/rerun_sdk` to `PYTHONPATH` or to a build rule (for example in Bazel) to work around the `.pth` file, remove it.

The `rerun` and `rerun-sdk` packages on PyPI install the same `rerun` directory, so do not install both.
