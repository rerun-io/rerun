# `rerun-notebook`

Part of the [Rerun](https://github.com/rerun-io/rerun) project.

## What?

`rerun-notebook` is a support package for [`rerun-sdk`](https://pypi.org/project/rerun-sdk/)'s notebook integration. This is an implementation package that shouldn't be directly interacted with. It is typically installed using the `notebook` [extra](https://packaging.python.org/en/latest/specifications/dependency-specifiers/#extras) of `rerun-sdk`:

```sh
pip install "rerun-sdk[notebook]"
```

## Why a separate package?

There are several reasons for this package to be separate from the main `rerun-sdk` package:

- `rerun-notebook` includes the JS+Wasm distribution of the Rerun viewer (~31MiB). Adding it to the main `rerun-sdk` package would double its file size.
- `rerun-notebook` uses [hatch](https://hatch.pypa.io/) as package backend, and benefits from the [hatch-jupyter-builder](https://github.com/jupyterlab/hatch-jupyter-builder) plug-in. Since `rerun-sdk` must use [Maturin](https://www.maturin.rs), it would make the package management more complex.
- Developer experience: building `rerun-notebook` implies building `rerun_js`, which is best avoided when iterating on `rerun-sdk` outside of notebook environments.

## Ways to access the widget assets

Even though `rerun_notebook` ships with the assets bundled in, by default it will try to load them from
`https://app.rerun.io`. This is because the way anywiget transmits the asset at the moment results in
[a memory leak](https://github.com/manzt/anywidget/issues/613) of the entire module for each cell execution.

If your network does not allow you to access `app.rerun.io`, the behavior can be changed by setting the
the `RERUN_NOTEBOOK_ASSET` environment variable before you import `rerun_notebook`. This variable must
be set prior to your import because `AnyWidget` stores the resource on the widget class instance
once at import time.

The assets are:
- `re_viewer_bg.wasm`, which is our Viewer compiled to Wasm, and
- `widget.js`, which is the glue code used to bind it to a Jupyter widget.

Both can be built in the [`rerun`](https://github.com/rerun-io/rerun) repository by running `pixi run py-build-notebook`.

### Inlined assets

Setting:
```
RERUN_NOTEBOOK_ASSET=inline
```
Will cause `rerun_notebook` to directly transmit the inlined assets to the widget over Jupyter comms.
This will be the most portable way to use the widget, but is currently known to leak memory and
has some performance issues in environments such as Google colab. The browser cannot cache the resulting
JS/Wasm, so it ends up spending a lot more time loading it in every output cell.

### Locally served assets

Setting:
```
RERUN_NOTEBOOK_ASSET=serve-local
```
Will cause `rerun_notebook` to launch a thread serving the assets from the local machine during
the lifetime of the kernel. This will be the best way to use the widget in a notebook environment
when your notebook server is running locally.

The JS and Wasm are served separately, so the Wasm can be stream-compiled, resulting in much faster
startup times. Both can also be cached by the browser.

### Manually hosted assets

Setting:
```
RERUN_NOTEBOOK_ASSET=https://your-hosted-asset-url.com/widget.js
```
Will cause `rerun_notebook` to load the assets from the provided URL. This is the most flexible way to
use the widget, but requires you to host the asset yourself.

Note that we require the URL to point to a `widget.js` file, but the Wasm file must be accessible from
a URL directly adjacent to it. Your server should provide both files:

- `https://your-hosted-asset-url.com/widget.js`
- `https://your-hosted-asset-url.com/re_viewer_bg.wasm`

The `rerun_notebook` package has a minimal server that can be used to serve the assets manually by running:
```
python -m rerun_notebook serve
```

However, any hosting platform can be used to serve the assets, as long as it is accessible to the notebook
and has appropriate CORS headers set. See: `asset_server.py` for a simple example.

## Run from source

Use Pixi:

```sh
# build rerun-sdk and rerun-notebook from source
pixi run py-build && pixi run py-build-notebook

# run jupyter
pixi run uv run jupyter notebook
```


## Development

Run the `pixi run py-build-notebook` build command any time you make changes to the Viewer or TypeScript code.
Changing python code only requires restarting the Jupyter kernel.
