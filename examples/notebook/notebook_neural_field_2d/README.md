<!--[metadata]
title = "Notebook: 2D neural fields"
tags = ["Notebook", "Neural Field", "2D"]
thumbnail = "https://static.rerun.io/tiger/b38c93f0efe8c5e7bd15270d8bc885128debcbae/480w.png"
thumbnail_dimensions = [480, 480]
channel = "nightly"
-->

https://vimeo.com/976650243?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1416:1244

## Overview

Rerun can be used directly within [Jupyter](https://jupyter.org/) notebooks.
Many additional environments beyond Jupyter are supported such as [Google Colab](https://colab.research.google.com/)
or [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks).

You need the separate support package [`rerun-notebook`](https://pypi.org/project/rerun-notebook/) to use this feature. Typically this is installed using:

```bash
pip install "rerun-sdk[notebook]"
```

Check out the [minimal notebook example](https://rerun.io/examples/integrations/notebook) for a quick start.

## Background

In this notebook we are fitting a simple neural field to a 2D image. The neural field is a simple multilayer perceptron with optional positional input encoding. The image is sampled uniformly and the network is trained to predict the color given the pixel position. To visualize the progress of the training we log the loss and regularly densely query the network to retrieve the image encoded in the network weights.

Using the notebook we can interactively try different learning rates, losses, and network architectures to see how they affect the training process.

## Running in Jupyter

The easiest way to try out the notebook is to use Jupyter.

First, install the requirements (this includes Jupyter, the Rerun SDK, and the notebook support package)

```
pip install -r requirements.txt
```

Then, open the notebook

```
jupyter notebook neural_field_2d.ipynb
```

Follow along in the browser that opens.
