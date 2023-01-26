# Python Docs

A high-level overview of writing and previewing the Rerun python documentation.

## Getting started with docs

### Dependencies
All of the dependencies for documentation generation are captured in the requirements file:
```
pip install -r rerun_py/requirements-doc.txt
```

### Serving the docs locally
The docs can be previewed locally using `mkdocs`

This will watch the contents of the `rerun_py` folder and refresh documentation live as files are changed.
```
mkdocs serve -f rerun_py/mkdocs.yml -w rerun_py
```
or
```
just serve-py-docs
```

### How versioned docs are generated and served
Our documentation is versioned with releases and generated via [mike](https://github.com/jimporter/mike)

The documentation exists as a [Github Pages](https://pages.github.com/) project which is hosted from the
contents of the `gh-pages` branch.

`mike` updates this branch with new content as part of CI and manually n release.

Every commit that lands to main will generate bleeding edge documentation as HEAD. Behind the scenes, a
github action is just running:
```
mike deploy -F rerun_py/mkdocs.yml HEAD
```

On release we instead deploy with a version tag:
```
mike deploy -F rerun_py/mkdocs.yml X.Y.Z latest
```

You can also locally preview the publicly hosted site wth all versions, using mike:
```
mike serve -F rerun_py/mkdocs.yml 
```
though when locally developing docs you are better off using `mkdocs serve` as described
above since it will handle hot-reloading for you as you edit.