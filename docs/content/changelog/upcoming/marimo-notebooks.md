---
title: Rerun Viewer in marimo notebooks
hidden: true
type: feature
---

### Rerun Viewer in marimo notebooks

The notebook Viewer now works in [marimo](https://marimo.io/) notebooks: `rr.notebook_show()`, `Viewer.display()`, and a `Viewer` or blueprint as the last expression of a cell all show an embedded Viewer.
marimo delivers messages from the Viewer only between cells, so to stream data live, create the Viewer in one cell and log to it from a later one.

[Embed Rerun in notebooks](../howto/integrations/embed-notebooks.md#running-in-marimo)
[marimo example notebook](https://github.com/rerun-io/rerun/blob/main/examples/notebook/notebook/cube_marimo.py)
