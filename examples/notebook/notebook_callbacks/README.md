<!--[metadata]
title = "Notebook: viewer callbacks"
tags = ["Notebook", "Interactive", "Callbacks", "3D"]
thumbnail = "https://static.rerun.io/notebook_callbacks/0daba8485bc0d589cfda3411db450db4bf2e8818/480w.png"
thumbnail_dimensions = [480, 339]
channel = "nightly"
-->

## Overview

This notebook demonstrates how to react to user interactions coming from the embedded Rerun Viewer widget. It logs a dynamic 3D point cloud, listens for timeline, time, and selection events, and surfaces them in real time using Jupyter widgets.

You need the separate support package [`rerun-notebook`](https://pypi.org/project/rerun-notebook/) to use this feature. Typically this is installed using:

```bash
pip install "rerun-sdk[notebook]"
```

Check out the [minimal notebook example](https://rerun.io/examples/integrations/notebook) for a quick start.

## Background

<!-- TODO(#11453): Add link to Viewer.on_event when page exists -->

This notebook spins up a colorful point cloud and pipes it into the viewer so you can experiment with callbacks in real time. As the camera, timeline, and selection change, `Viewer.on_event` emits rich event payloads that we translate into friendly [`ipywidgets`](https://ipywidgets.readthedocs.io/) readouts.

Scrub the timeline, pick individual points, or activate entire views to see how each interaction updates the labels—handy for building responsive dashboards or debugging custom tooling around the Rerun Viewer.

## Running in Jupyter

First, install the requirements (this includes Jupyter, the Rerun SDK, and the notebook support package):

```bash
pip install -r requirements.txt
```

Then, open the notebook:

```bash
jupyter notebook notebook_callbacks.ipynb
```

Interact with the viewer by scrubbing the timeline and selecting points or views; the widgets underneath will update instantly to mirror the viewer state.
