---
title: Python SDK
order: 100
---

The Python SDK includes both the SDK and the Viewer, so you're ready to go with a single install:

-   `pip install rerun-sdk` via pip
-   `conda install -c conda-forge rerun-sdk` via Conda

<!-- NOLINT_START -->

Conda always comes with support for all features but if using pip you may need to specify optional features:
-   `pip install rerun-sdk[notebook]` for the embedded notebook tools
-   `pip install rerun-sdk[dataplatform]` for the query api tools

<!-- NOLINT_END -->

## Next steps

To start getting your own data streamed to the viewer, check out the [Python quick start guide](../../getting-started/data-in/python.md).
