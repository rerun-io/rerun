<!--[metadata]
title = "Notebook: minimal example"
tags = ["Notebook", "API example", "3D"]
thumbnail = "https://static.rerun.io/notebook_cube/e40da7048fb5c0c12ef9931225791c27469d057d/480w.png"
thumbnail_dimensions = [480, 480]
channel = "nightly"
-->

<picture>
  <img src="https://static.rerun.io/notebook_cube_header/fabe5fca9bdbce54badfbdcb41c50a984edcf829/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/notebook_cube_header/fabe5fca9bdbce54badfbdcb41c50a984edcf829/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/notebook_cube_header/fabe5fca9bdbce54badfbdcb41c50a984edcf829/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/notebook_cube_header/fabe5fca9bdbce54badfbdcb41c50a984edcf829/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/notebook_cube_header/fabe5fca9bdbce54badfbdcb41c50a984edcf829/1200w.png">
</picture>

## Overview

Rerun can be used directly within [Jupyter](https://jupyter.org/) notebooks.
Many additional environments beyond Jupyter are supported such as [Google Colab](https://colab.research.google.com/)
or [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks).

You need the separate support package [`rerun-notebook`](https://pypi.org/project/rerun-notebook/) to use this feature. Typically this is installed using:

```bash
pip install "rerun-sdk[notebook]"
```

In order to show a Rerun Viewer inline within a notebook, you can call:

```python
rr.init("rerun_example_notebook")

rr.log(...)

rr.notebook_show()
```

This will show the contents of the current global recording stream. Note that the global stream will accumulate
data in-memory. You can reset the stream by calling `rr.init` again to establish a new global context.

As with the other stream viewing APIs (`rr.show`, `rr.connect_grpc`, `rr.spawn`), you can alternatively pass
a specific recording instance to `notebook_show`

```python
rec = rr.RecordingStream("rerun_example_notebook_local")

rec.log(...)
rec.notebook_show()
```

## Running in Jupyter

The easiest way to get a feel for working with notebooks is to use it.

First, install the requirements (this includes Jupyter, the Rerun SDK, and the notebook support package)

```sh
pip install -r requirements.txt
```

Then, open the notebook

```sh
jupyter notebook cube.ipynb
```

Follow along in the browser that opens.
