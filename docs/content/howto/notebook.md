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
This method embeds the [web viewer](./embed-rerun-viewer.md) using the IPython `display` mechanism
in the cell output, and sends the current recording data to it.
Once the viewer is open, any subsequent `rr.log()` calls will send their data directly to the viewer,
without any intermediate buffering.

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

This is similar to calling `rr.connect()` or `rr.serve()` in that it configures the Rerun SDK to send data to a viewer instance.

Note that the call to `rr.notebook_show()` drains the recording recording of its data. This means that any subsequent calls to `rr.notebook_show()`
will not result in the same data being displayed, because it has already been removed from the recording.
Support for this is tracked in [#6612](https://github.com/rerun-io/rerun/issues/6612).

If you wish to start a new recording, you can call `rr.init()` again.

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

Because blueprint types implement `_ipython_display_`, you can also just end any cell with a blueprint
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

## Limitations

Browsers have limitations on the amount of memory usable by a single tab. If you are working with large datasets,
you may run into issues where the browser tab crashes. If you encounter this, you can try to use the `save()` API
to save the data to a file and share it as a standalone asset.

## Future work

We are actively working on improving the notebook experience and welcome any feedback or suggestions.
The ongoing roadmap is being tracked in [GitHub issue #1815](https://github.com/rerun-io/rerun/issues/1815).
