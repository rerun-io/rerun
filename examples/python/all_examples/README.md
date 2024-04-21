# Example meta-project

TODO(ab): this is largely WIP with unclear path to actual usefulness.

## What?

### Dynamically depend on all example

Running `pip install -e .` will transitively install all examples that are compatible with the current Python version and platform. The examples can then be run directly:

```shell
clock  # runs the clock example
python -m clock  # this is also possible
```

The dynamic dependency list is achieved in `hatch_build.py`, which is registered as a hook. This hook adds [environment marker](https://packaging.python.org/en/latest/specifications/dependency-specifiers/#environment-markers) to mark Python version or platform restrictions.


### List examples


Running `python -m all_examples list` prints a list of all examples, suitable for copy-pasting to the `pixi.toml` file.
