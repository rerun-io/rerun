## Example meta-project

TODO(ab): this is largely WIP with unclear path to actual usefulness.

### Dynamically depend on all examples

This project dynamically depend on all examples.

Running `pip install -e .` will transitively install all examples that are compatible with the current Python version and platform. The examples can then be run directly:

```shell
clock  # runs the clock example
python -m clock  # this is also possible
```

This is useful to rapidly test for potentially conflicting dependencies across examples:
```shell
cd /tmp
uv venv
source .venv/bin/activate
uv pip install -e path/to/rerun/examples/python/all_examples   # ok??
```

The dynamic dependency list is achieved in `hatch_build.py`, which is registered as a hook. This hook adds [environment marker](https://packaging.python.org/en/latest/specifications/dependency-specifiers/#environment-markers) to mark Python version or platform restrictions.


### List examples

Running `python -m all_examples list` prints a list of all examples, suitable for copy-pasting to the `pixi.toml` file.

### Configuration

`all_examples` can be configured via the `[tool.rerun-example]` table from the `pyproject.toml` file. It currently supports the following parameters:

```toml
[tool.rerun-example]
# always ignore this example
skip = true

# specify extra arguments when running this example (not yet used)
extra-args = "--dataset hello_world"  # may also be a list

# specify platform(s) incompatible with this example
# will be translated into a `sys_platform` environment marker
exclude-platform = "darwin"  # may also be a list
```

In addition, `all_examples` reads the `requires-python` setting from the `[project]` table (if any), and translate it to `python_version` environment marker.
