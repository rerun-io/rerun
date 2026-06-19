---
title: Python SDK
order: 100
---

The Python SDK includes both the SDK and the Viewer, so you're ready to go with a single install:

-   `pip install rerun-sdk` via pip
-   `conda install -c conda-forge rerun-sdk` via Conda

Conda always comes with support for all features but if using pip you may need to specify optional features:
-   `pip install rerun-sdk[notebook]` for the embedded notebook tools
-   `pip install rerun-sdk[catalog]` for the query api tools
-   `pip install rerun-sdk[dataloader]` for model training tools

## Next steps

[Set up a Python project](../../getting-started/project-setup/python.md), then walk through the [Log and Ingest](../../getting-started/data-in.md) tutorial.
