---
title: Using Rerun with Notebooks
order: 3
description: How to embed Rerun in notebooks like Jupyter or Colab
---

Starting with version 0.5.0, Rerun now has limited support for embedding the Rerun viewer directly within IPython-style
notebooks. This makes it easy to iterate on API calls as well as to share data with others.

Rerun has been tested with:
 - [Jupyter Notebook Classic](https://jupyter.org/)
 - [Jupyter Lab](https://jupyter.org/)
 - [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks)
 - [Google Colab](https://colab.research.google.com/)

## Basic Concept

Rather than logging to a file or a remote server, you can also configure the Rerun SDK to store data in a local
[MemoryRecording](https://ref.rerun.io/docs/python/stable/common/other_classes_and_functions/#rerun.MemoryRecording).

This `MemoryRecording` can then used to produce an inline HTML snippet to be directly displayed in most notebook
environments. The snippet includes an embedded copy of an RRD file and some javascript that loads that RRD file into an
IFrame.

Each cell in the notebook is fully isolated from the other cells and will only display the data from the source
`MemoryRecording`.

## The APIs

In order to create a new `MemoryRecording`, you call:
```python
rec = rr.memory_recording()
```
This is similar to calling `rr.connect()` or `rr.save()` in that it configures the Rerun SDK to use this new
recording as a target for future API calls.

After logging data to the recording you can display it in a cell by calling the
[show()](https://ref.rerun.io/docs/python/stable/common/other_classes_and_functions/#rerun.MemoryRecording.show) method
on the `MemoryRecording`. The `show()` method also takes optional arguments for specifying the width and height of the IFrame. For example:
```python
rec.show(width=400, height=400)
```

The `MemoryRecording` also implements `_repr_html_()` which means in most notebook environments, if it is the last
expression returned in a cell it will display itself automatically, without the need to call `show()`.
```python
rec = rr.memory_recording()
rr.log("img", my_image)
rec
```
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

Because the Rerun viewer in the notebook is just an embedded HTML snippet it also works with
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

## Future Work

We are actively working on improving the notebook experience and welcome any feedback or suggestions.
The ongoing roadmap is being tracked in [GitHub issue #1815](https://github.com/rerun-io/rerun/issues/1815).
