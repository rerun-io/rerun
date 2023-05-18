# Overview

Rerun has limited support for running directly embedded within a [jupyter][https://jupyter.org/] notebook.
Many additional environments beyond jupyter are supported such as [Google Colab][https://colab.research.google.com/]
or [VSCode](https://code.visualstudio.com/blogs/2021/08/05/notebooks).

In order to show a rerun viewer inline within the notebook you need to use a special in-memory
recording:
```
rec = viewer.memory_recording()
```

After creating this recording all the normal rerun commands will work as expected and log
to this recording instance. When you are ready to show it you can return it at the end of your cell
or call `rec.show()`.

# Running in Jupyter

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
