r"""
Generate the code reference pages and navigation for the source tree.

This helper is used by the [mkdocs-gen-files plugin](https://oprypin.github.io/mkdocs-gen-files)

When building the documentation it will walk our python source-tree and create a collection
of virtual files, as well as a `SUMMARY.txt` for use with the
[mkdocs-literature-nav plugin](https://oprypin.github.io/mkdocs-literate-nav)

Example virtual file:

`package/rerun/log/points.md`:
```
::: rerun.log.points
```

`SUMMARY.txt`:
```
* [index](index.md)
* rerun/
    * [\\__init__.py](rerun/__init__.md)
    * [color_conversion.py](rerun/color_conversion.md)
    * components/
        * [\\__init__.py](rerun/components/__init__.md)
        * [annotation.py](rerun/components/annotation.md)
        * [arrow.py](rerun/components/arrow.md)
        * [box.py](rerun/components/box.md)
        * [color.py](rerun/components/color.md)
        * [instance.py](rerun/components/instance.md)
```

"""

from pathlib import Path

import mkdocs_gen_files

root = Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()
package_dir = Path("package")

nav = mkdocs_gen_files.Nav()
nav["index"] = "index.md"

for package in ["depthai_viewer", "rerun_demo"]:
    for path in sorted(root.joinpath(package).rglob("*.py")):
        rel_path = path.relative_to(root)

        # Build up a python-style dotted identifier for the module
        # This is how `mkdocstrings-python` will import the module and also how
        # we refer to it with links internal to the docs
        module_path = rel_path.with_suffix("")
        ident_parts = tuple(module_path.parts)
        if ident_parts[-1] == "__init__":
            ident_parts = ident_parts[:-1]
        elif ident_parts[-1] == "__main__":
            continue
        ident = ".".join(ident_parts)

        # The doc_path is the md file that we will generate inside the virtual package folder
        doc_path = rel_path.with_suffix(".md")
        write_path = package_dir.joinpath(doc_path)

        # Within the nav, we want non-leaf nodes to appear as folders
        nav_parts = tuple(p + "/" for p in rel_path.parts[:-1]) + (path.parts[-1],)

        # Register the nav-parts index with the generated doc-path
        nav[nav_parts] = doc_path.as_posix()

        # Write the virtual file
        with mkdocs_gen_files.open(write_path, "w") as fd:
            fd.write(f"::: {ident}")

# Generate the SUMMARY.txt file
with mkdocs_gen_files.open(package_dir.joinpath("SUMMARY.txt"), "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
