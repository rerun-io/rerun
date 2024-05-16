# Python docs

A high-level overview of writing and previewing the Rerun Python documentation.

## Getting started with docs

### Dependencies
All of the dependencies for documentation generation are captured in the requirements file:
```
pixi run pip install -r rerun_py/requirements-doc.txt
```

### Serving the docs locally
This will watch the contents of the `rerun_py` folder and refresh documentation live as files are changed.
```sh
pixi run py-docs-serve
```

### How versioned docs are generated and served
Our documentation is versioned with releases and generated via [mike](https://github.com/jimporter/mike)

The documentation exists as a [GitHub Pages](https://pages.github.com/) project which is hosted from the
contents of the `gh-pages` branch.

`mike` updates this branch with new content as part of CI

Every commit that lands to main will generate bleeding edge documentation as HEAD. Behind the scenes, a
GitHub action is just running:
```sh
pixi run mike deploy -F rerun_py/mkdocs.yml HEAD
```

On release, when GitHub sees a new tag: `X.Y.Z`, the GitHub action will instead deploy with a version tag:
```sh
pixi run mike deploy -F rerun_py/mkdocs.yml X.Y.Z latest
```

You can also locally preview the publicly hosted site with all versions, using mike:
```sh
pixi run mike serve -F rerun_py/mkdocs.yml
```
though when locally developing docs you are better off using `mkdocs serve` as described
above since it will handle hot-reloading for you as you edit.
