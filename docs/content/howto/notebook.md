---
title: Embed Rerun in notebooks
order: 600
description: How to embed Rerun in notebooks like Jupyter or Colab
---

Starting with version 0.15.1, Rerun has improved support for embedding the Rerun Viewer directly within IPython-style
notebooks. This makes it easy to iterate on API calls as well as to share data with others.

Rerun has been tested with:

-   [Jupyter Notebook Classic](https://jupyter.org/)
-   [Jupyter Lab](https://jupyter.org/)
-   [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks)
-   [Google Colab](https://colab.research.google.com/)

## Basic concept

When using the Rerun logging APIs, by default, the logged messages are buffered in-memory until
you send them to a sink such as via `rr.connect()` or `rr.save()`. When using Rerun in a notebook,
rather than using the other sinks, you have the option to use a helper method: [`rr.notebook_show()`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.notebook_show).
This method takes any buffered messages and converts them into an HTML snipped including
the inlined data along with an instance of the Viewer in an iframe.

## The APIs

In order to output the current recording data to a notebook cell, call:
[`rr.notebook_show()`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.notebook_show).

For example:

```python
import rerun as rr
from numpy.random import default_rng

rr.init("rerun_example_notebook")

rng = default_rng(12345)

positions = rng.uniform(-5, 5, size=[10, 3])
colors = rng.uniform(0, 255, size=[10, 3])
radii = rng.uniform(0, 1, size=[10])

rr.log("random", rr.Points3D(positions, colors=colors, radii=radii))

rr.notebook_show()
```

<picture>
  <img src="https://static.rerun.io/notebook_example/e47920b7ca7988aba305d73b2aea2da7b81c93e3/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/notebook_example/e47920b7ca7988aba305d73b2aea2da7b81c93e3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/notebook_example/e47920b7ca7988aba305d73b2aea2da7b81c93e3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/notebook_example/e47920b7ca7988aba305d73b2aea2da7b81c93e3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/notebook_example/e47920b7ca7988aba305d73b2aea2da7b81c93e3/1200w.png">
</picture>

This is similar to calling `rr.connect()` or `rr.save()` in that it configures the Rerun SDK to use
this memory buffer as the sink for future logging calls.

Note that the output cell is essentially a fixed snapshot of the
current state of the recording at the time that `notebook_show()` is called. Rerun does not yet
support live incremental streaming from the Jupyter kernel into the embedded viewer.

Messages will continue to be buffered incrementally, and each call to `notebook_show()` will
display all messages that have been logged since the last call to `rr.init()`.

If you wish to clear the current recording, you can call `rr.init()` again.

The `notebook_show()` method also takes optional arguments for specifying the width and height of the IFrame. For example:

```python
rr.notebook_show(width=400, height=400)
```

## Working with blueprints

[Blueprints](./configure-viewer-through-code.md) can also be used with `notebook_show()` by providing a `blueprint`
parameter.

For example

```python
blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(origin="/world"),
        rrb.Spatial2DView(origin="/world/camera"),
        column_shares=[2,1]),
)

rr.notebook_show(blueprint=blueprint)
```

Because blueprint types implement `_repr_html_`, you can also just end any cell with a blueprint
object, and it will call `notebook_show()` behind the scenes.

```python
import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_image")
rng = np.random.default_rng(12345)

image1 = rng.uniform(0, 255, size=[24, 64, 3])
image2 = rng.uniform(0, 255, size=[24, 64, 1])

rr.log("image1", rr.Image(image1))
rr.log("image2", rr.Image(image2))

rrb.Vertical(
    rrb.Spatial2DView(origin='/image1'),
    rrb.Spatial2DView(origin='/image2')
)
```

<picture>
  <img src="https://static.rerun.io/notebook_blueprint_example/eb0663a9a8a0de8276390667a774acc1bc86148e/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/notebook_blueprint_example/eb0663a9a8a0de8276390667a774acc1bc86148e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/notebook_blueprint_example/eb0663a9a8a0de8276390667a774acc1bc86148e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/notebook_blueprint_example/eb0663a9a8a0de8276390667a774acc1bc86148e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/notebook_blueprint_example/eb0663a9a8a0de8276390667a774acc1bc86148e/1200w.png">
</picture>

## Some working examples

To experiment with notebooks yourself, there are a few options.

### Running locally

The GitHub repo includes a [notebook example](https://github.com/rerun-io/rerun/blob/main/examples/python/notebook/cube.ipynb).

If you have a local checkout of Rerun, you can:

```bash
$ cd examples/python/notebook
$ pip install -r requirements.txt
$ jupyter notebook cube.ipynb
```

This will open a browser window showing the notebook where you can follow along.

### Running in Google Colab

We also host a copy of the notebook in [Google Colab](https://colab.research.google.com/drive/1R9I7s4o6wydQC_zkybqaSRFTtlEaked_)

Note that if you copy and run the notebook yourself, the first Cell installs Rerun into the Colab environment.
After running this cell you will need to restart the Runtime for the Rerun package to show up successfully.

## Sharing your notebook

Because the Rerun Viewer in the notebook is just an embedded HTML snippet it also works with
tools like nbconvert.

You can convert the notebook to HTML using the following command:

```bash
$ jupyter nbconvert --to=html --ExecutePreprocessor.enabled=True examples/python/notebook/cube.ipynb
```

This will create a new file `cube.html` that can be hosted on any static web server.

[Example cube.html](https://static.rerun.io/93d3f93e0951b2e2fedcf70f71014a3b3a5e8ef6_cube.html)

## Limitations

Although convenient, the approach of fully inlining an RRD file as an HTML snippet has some drawbacks. In particular,
it is not suited to large RRD files. The RRD file is embedded as a base64 encoded string which can
result in a very large HTML file. This can cause problems in some browsers. If you want to share large datasets,
we recommend using the `save()` API to create a separate file and hosting it as a separate standalone asset.

## Future work

We are actively working on improving the notebook experience and welcome any feedback or suggestions.
The ongoing roadmap is being tracked in [GitHub issue #1815](https://github.com/rerun-io/rerun/issues/1815).
