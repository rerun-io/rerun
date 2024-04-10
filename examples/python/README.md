# Rerun Python examples

---

## Example dependency management

We need the following for a smooth example experience:
- Per-example dependency specification: different example must be able to have conflicting dependencies.
- Per-example Python version requirement: although the Rerun SDK has a well-defined Python compatibility range, specific example should be able to restrict it when required by their specific dependencies.
- Per-example isolated environment: it follows from what precedes that each example should be run in their own venv.
- Minimal overhead and as "standard" as possible.
- (Nice to have) Lockfile.

In addition, we should support both dev environment (install `rerun-sdk` from the working directory) and end-user environment (install `rerun-sdk` from PyPI).

---

## Using `uv`

Example: `human_pose_tracking`

### Tooling Installation

```shell
pipx install uv

# or

curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Environment Setup

Must be manually created but `uv` has facilities for that:

```shell
cd example/python/human_pose_tracking
uv venv --python 3.11 
source .venv/bin/activate
uv pip install -r requirements.txt

# to use local rerun-sdk (this triggers a maturin compilation)
uv pip install -e ../../../rerun_py
```

⚠️There don't seem to be a way to specify Python version requirement for `uv` yet, so the user must pick a compatible python version.

Note: since this is basically based on a (locked) requirement file, "standard" Python tooling will also work:

```shell
cd example/python/human_pose_tracking
python3.11 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
pip install -e ../../../rerun_py
```

In that sense, `uv` is only required for example authors (to update the `requirements.txt` file).

### Run the example

```shell
python main.py
```

### Create/update the lockfile

The direct dependencies are specified in `requirements.in`. From that, `uv` is able to generate a `requirements.txt` which essentially is a lockfile:

```shell
uv pip compile requirements.in -o requirements.txt
```

---

## Using `poetry` with `package-mode = false`

Example: `human_pose_tracking_poetry`

Note: `package-mode = false` is very nice for lone scripts that don't need to be "pip installed" to run, just like our examples.  

### Tooling Installation

```shell
pipx install poetry

# or

curl -sSL https://install.python-poetry.org | python3 -
```

### Environment Setup

```shell
poetry install

# to use the local rerun-sdk (this triggers maturin compilation)
poetry run pip install -e ../../../rerun_py
```

This will do the following:
- Unless a venv is already active, it will create one using some python interpreter that it can find and is compatible with constraints listed in `pyproject.toml`.
- Install all dependencies.
- Create the `main` executable script as specified in the `pyproject.toml`.

Additional facilities provided by Poetry:
```shell
poetry run ${ARGS}  #  run stuff inside the default environment
poetry env use 3.8  # create another environment with the specified python version
poetry env list     # list all available environments
poetry shell        # activate the default environment
```

### Run the example

```shell
poetry run python main.py

# or

poetry shell  # activate the venv
python main.py
```

### Create/update the lockfile

```shell
poetry update
```

---

## Using `hatch`

Example: `human_pose_tracking_hatch`

`hatch` is similar to `poetry`, but with wider scope and a richer plug-in ecosystem to support it.

### Tooling Installation

```shell
pipx install hatch
```

### Environment Setup

Basically nothing, `hatch` creates a venv on the fly.

To use the dev rerun-sdk:
```shell
hatch run pip install ../../../rerun_py
```

### Run the example

```shell
hatch run python main.py

# or

hatch shell
python main.py 
```




---
_original content follows_

The simplest example is [`minimal`](minimal/main.py). You may want to start there!

Read more about our examples at <https://www.rerun.io/examples>.

## Setup
First install the Rerun Python SDK with `pip install rerun-sdk` or via [`conda`](https://github.com/conda-forge/rerun-sdk-feedstock).

> Note: Make sure your SDK version matches the code in the examples.
For example, if your SDK version is `0.4.0`, check out the matching tag
for this repository by running `git checkout v0.4.0`.

## Dependencies
Each example comes with its own set of dependencies listed in a `requirements.txt` file. For example, to install dependencies and run the toy `minimal` example (which doesn't need to download any data) run:

```sh
pip install -r examples/python/minimal/requirements.txt
python examples/python/minimal/main.py
```

You can also install all dependencies needed to run all examples with:

```sh
pip install -r examples/python/requirements.txt
```

## Running the examples
By default, the examples spawn a Rerun Viewer and stream log data to it.

For most examples you can instead save the log data to an `.rrd` file using `examples/python/plots/main.py --save data.rrd`. You can then view that `.rrd` file with `rerun data.rrd`.

(`rerun` is an alias for `python -m rerun`).

NOTE: `.rrd` files do not yet guarantee any backwards or forwards compatibility. One version of Rerun will likely not be able to open an `.rrd` file generated by another Rerun version.

## Datasets
Some examples will download a small datasets before they run. They will do so the first time you run the example. The datasets will be added to a subdir called `dataset`, which is in the repo-wide `.gitignore`.

## Contributions welcome
Feel free to open a PR to add a new example!

See [`CONTRIBUTING.md`](../../CONTRIBUTING.md) for details on how to contribute.
