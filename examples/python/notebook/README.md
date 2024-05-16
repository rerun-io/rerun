## Overview

Rerun has limited support for direct embedding within a [Jupyter](https://jupyter.org/) notebook.
Many additional environments beyond Jupyter are supported such as [Google Colab](https://colab.research.google.com/)
or [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks).

In order to show a Rerun Viewer inline within the notebook, you can call:

```python
rr.init("rerun_example_notebook")

rr.log(...)

rr.notebook_show()
```

This will show the contents of the current global recording stream. Note that the global stream will accumulate
data in-memory. You can reset the stream by calling `rr.init` again to establish a new global context.

As with the other stream viewing APIs (`rr.show`, `rr.connect`, `rr.spawn`), you can alternatively pass
a specific recording instance to `notebook_show`

```python
rec = rr.new_recording("rerun_example_notebook_local")

rec.log(...)

rr.notebook_show(recording=rec)
```

## Running in Jupyter

The easiest way to get a feel for working with notebooks is to use it:

Install jupyter

```
pip install -r requirements.txt
```

Open the notebook

```
jupyter notebook cube.ipynb
```

Follow along in the browser that opens.
