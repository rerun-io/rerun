r"""
Generate an index table and rendered pages for the common APIs.

The top-level index file should look like
```
## Initialization
Function | Description
-------- | -----------
[rerun.init()](initialization/#rerun.init) | Initialize the Rerun SDK ...
[rerun.set_recording_id()](initialization/#rerun.set_recording_id) | Set the recording ID ...
[rerun.connect()](initialization/#rerun.connect) | Connect to a remote Rerun Viewer on the ...
[rerun.spawn_and_connect()](initialization/#rerun.spawn_and_connect) | Spawn a Rerun Viewer ...

...

The Summary should look like:
```
* [index](index.md)
* [Initialization](initialization.md)
* [Logging Primitives](primitives.md)
* [Logging Images](images.md)
* [Annotations](annotation.md)
* [Extension Components](extension_components.md)
* [Plotting](plotting.md)
* [Transforms](transforms.md)
```
"""

import re
import sys
from pathlib import Path
from typing import Final, List, Tuple

import griffe
import mkdocs_gen_files

# This is the list of sections and functions that will be included in the index
# for each of them.
SECTION_TABLE: Final[List[Tuple[str, List[str]]]] = [
    (
        "Initialization",
        ["init", "set_recording_id", "connect", "spawn_and_connect"],
    ),
    (
        "Logging Primitives",
        ["log_point", "log_points", "log_rect", "log_rects", "log_obb", "log_path", "log_line_segments", "log_arrow"],
    ),
    (
        "Logging Images",
        ["log_image", "log_image_file", "log_depth_image", "log_segmentation_image"],
    ),
    (
        "Annotations",
        ["log_annotation_context"],
    ),
    (
        "Extension Components",
        ["log_extension_components"],
    ),
    (
        "Plotting",
        ["log_scalar"],
    ),
    (
        "Transforms",
        ["log_rigid3", "log_pinhole", "log_unknown_transform", "log_view_coordinates"],
    ),
]

# Virual folder where we will generate the md files
root = Path(__file__).parent.parent.resolve()
common_dir = Path("common")

# We use griffe to access docstrings
# Lots of other potentially interesting stuff we could pull out in the future
# This is what mkdocstrings uses under the hood
search_paths = [path for path in sys.path if path]  # eliminate empty path
search_paths.insert(0, root.as_posix())
rerun_pkg = griffe.load("rerun", search_paths=search_paths)

# Create the nav for this section
nav = mkdocs_gen_files.Nav()
nav["index"] = "index.md"

# This is the top-level index which will include a table-view of each sub-section
index_path = common_dir.joinpath("index.md")


def make_slug(s: str) -> str:
    s = s.lower().strip()
    s = re.sub(r"[\s]+", "_", s)
    return s


with mkdocs_gen_files.open(index_path, "w") as index_file:

    # Hide the TOC for the index since it's identical to the left nav-bar
    index_file.write(
        """---
hide:
    - toc
---
# Common APIs
"""
    )

    for (heading, func_list) in SECTION_TABLE:

        # Turn the heading into a slug and add it to the nav
        md_name = make_slug(heading)
        md_file = md_name + ".md"
        nav[heading] = md_file

        # Write out the contents of this section
        write_path = common_dir.joinpath(md_file)
        with mkdocs_gen_files.open(write_path, "w") as fd:
            for func_name in func_list:
                fd.write(f"::: rerun.{func_name}\n")

        # Write out a table for the section in the index_file
        index_file.write(f"## {heading}\n")
        index_file.write("Function | Description\n")
        index_file.write("-------- | -----------\n")
        for func_name in func_list:
            func = rerun_pkg[func_name]
            index_file.write(f"[`rerun.{func_name}()`]({md_name}#rerun.{func_name}) | {func.docstring.lines[0]}\n")
        index_file.write("\n")


# Generate the SUMMARY.txt file
with mkdocs_gen_files.open(common_dir.joinpath("SUMMARY.txt"), "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
