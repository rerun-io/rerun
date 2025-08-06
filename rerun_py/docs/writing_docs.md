# Python docs

A high-level overview of writing and previewing the Rerun Python documentation.

## Getting started with docs

### Serving the docs locally
This will watch the contents of the `rerun_py` folder and refresh documentation live as files are changed.
```sh
pixi run py-docs-serve
```

TODO(#7913): You may need to delete `rerun_py/rerun_bindings/rerun_bindings.abi3.so`.

### How versioned docs are generated and served
Our documentation is versioned with releases and generated via [mkdocs](https://github.com/mkdocs/mkdocs)

The documentation exists as bucket on GCS which is hosted on the <https://ref.rerun.io> domain.

Every commit that lands to main will generate bleeding edge documentation as HEAD. Behind the scenes, a
GitHub action is running `pixi run -e py-docs py-docs-build`, and uploading the result to GCS at
[`docs/python/main`](https://ref.rerun.io/docs/python/main).

Releases will push to a version instead: [`docs/python/0.23.3`](https://ref.rerun.io/docs/python/0.23.3)
