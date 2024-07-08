# `rerun-notebook`

Part of the [Rerun](https://github.com/rerun-io/rerun) project.

## What?

`rerun-notebook` is a support package for [`rerun-sdk`](https://pypi.org/project/rerun-sdk/)'s notebook integration. This is an implementation package that shouldn't be directly interacted with. It is typically installed using the `notebook` [extra](https://packaging.python.org/en/latest/specifications/dependency-specifiers/#extras) of `rerun-sdk`:

```sh
pip install "rerun-sdk[notebook]"
```

## Why a separate package?

There are several reasons for this package to be separate from the main `rerun-sdk` package:

- `rerun-notebook` includes the JS distribution of the Rerun viewer (~31MiB). Adding it to the main `rerun-sdk` package would double its file size.
- `rerun-notebook` uses [hatch](https://hatch.pypa.io/) as package backend, and benefits from the [hatch-jupyter-builder](https://github.com/jupyterlab/hatch-jupyter-builder) plug-in. Since `rerun-sdk` must use [Maturin](https://www.maturin.rs), it would make the package management more complex.
- Developer experience: building `rerun-notebook` implies building `rerun_js`, which is best avoided when iterating on `rerun-sdk` outside of notebook environments.

## Ways to access the widget asset

Even though `rerun_notebook` ships with the widget asset bundled in, by default it will try to load the asset
from `https://app.rerun.io`. This is because the way anywiget transmits the asset at the moment results in
[a memory leak](https://github.com/manzt/anywidget/issues/613) of the entire module for each cell execution.

If your network does not allow you to access `app.rerun.io`, the behavior can be changed by setting the
the `RERUN_NOTEBOOK_ASSET` environment variable before you import `rerun_notebook`. This variable must
be set prior to your import because `AnyWidget` stores the resource on the widget class instance
once at import time.

### Inlined asset
Setting:
```
RERUN_NOTEBOOK_ASSET=inline
```
Will cause `rerun_notebook` to directly transmit the inlined asset to the widget over Jupyter comms.
This will be the most portable way to use the widget, but is currently known to leak memory and
has some performance issues in environments such as Google colab.

### Locally served asset
Setting:
```
RERUN_NOTEBOOK_ASSET=serve-local
```
Will cause `rerun_notebook` to launch a thread serving the asset from the local machine during
the lifetime of the kernel. This will be the best way to use the widget in a notebook environment
when your notebook server is running locally.

### Manually hosted asset
Setting:
```
RERUN_NOTEBOOK_ASSET=https://your-hosted-asset-url.com/widget.js
```
Will cause `rerun_notebook` to load the asset from the provided URL. This is the most flexible way to
use the widget, but requires you to host the asset yourself.

The `rerun_notebook` package has a minimal server that can be used to serve the asset nanually by running:
```
python -m rerun_notebook serve
```

However, any hosting platform can be used to serve the asset, as long as it is accessible to the notebook
and has appropriate CORS headers set. See: `asset_server.py` for a simple example.

## Run from source

Use Pixi:

```sh
# install rerun-sdk from source with the "notebook" extra
pixi run -e examples py-build-notebook

# run jupyter
pixi run -e examples jupyter notebook
```


## Development

Create a virtual environment and install `rerun-notebook` in *editable* mode with the
optional development dependencies:

```sh
python -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

You then need to install the JavaScript dependencies and run the development server.

```sh
npm install
npm run dev
```

Open `example.ipynb` in JupyterLab, VS Code, or your favorite editor
to start developing. Changes made in `js/` will be reflected
in the notebook.
