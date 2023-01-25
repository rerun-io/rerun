# Python Docs

A high-level overview of writing and previewing the Rerun python documentation.

## Getting started with docs

### Dependencies
All of the dependencies for documentation generation are captured in the requirements file:
```
pip install -r rerun_py/requirements-doc.txt
```

### Serving the docs
The docs can be previewed locally using `mkdocs`

This will watch the contents of the `rerun_py` folder and refresh documentation live as files are changed.
```
mkdocs serve -f rerun_py/mkdocs.yml -w rerun_py
```
or
```
just serve-py-docs
```
