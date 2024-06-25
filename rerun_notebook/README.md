# `rerun_notebook`

## What?

`rerun_notebook` is a support package for `rerun_sdk`'s notebook integration. This is an implementation package that shouldn't be directly interacted with. It is typically installed using the `notebook` [extra](https://packaging.python.org/en/latest/specifications/dependency-specifiers/#extras) of `rerun_sdk`:

```sh
pip install "rerun_sdk[notebook]"
```

## Why?

There are several reasons for this package to be separate from the main `rerun_sdk` package:

- `rerun_notebook` includes the JS distribution of the Rerun viewer (~31MiB). Adding it to the main `rerun_sdk` package would double its file size.
- `rerun_notebook` uses [hatch](https://hatch.pypa.io/) as package backend, and benefits from the [hatch-jupyter-builder](https://github.com/jupyterlab/hatch-jupyter-builder) plug-in. Since `rerun_sdk` must use [Maturin](https://www.maturin.rs), it would the package management more complex.
- Developer experience: building `rerun_notebook` implies building `rerun_js`, which is best avoided when iterating on `rerun_sdk` outside of notebook environments.

## Run from source

Use pixi:

```sh
# install rerun_sdk from source with the "notebook" extra
pixi run -e examples py-build-notebook

# run jupyter
pixi run -e examples jupyter notebook
```


## Development

Create a virtual environment and install rerun_notebook in *editable* mode with the
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
