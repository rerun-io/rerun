r"""
Generate an index table and rendered pages for the common APIs.

The top-level index file should look like
```
## Initialization
Function | Description
-------- | -----------
[depthai_viewer.init()](initialization/#depthai_viewer.init) | Initialize the Rerun SDK ...
[depthai_viewer.set_recording_id()](initialization/#depthai_viewer.set_recording_id) | Set the recording ID ...
[depthai_viewer.connect()](initialization/#depthai_viewer.connect) | Connect to a remote Depthai Viewer on the ...
[depthai_viewer.spawn()](initialization/#depthai_viewer.spawn) | Spawn a Depthai Viewer ...
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
* [Helpers](helpers.md)
```
"""

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Final, List, Optional

import griffe
import mkdocs_gen_files


@dataclass
class Section:
    title: str
    module_summary: Optional[str]
    func_list: List[str]


# This is the list of sections and functions that will be included in the index
# for each of them.
SECTION_TABLE: Final[List[Section]] = [
    Section(
        title="Initialization",
        module_summary=None,
        func_list=["init", "connect", "disconnect", "spawn", "serve", "memory_recording"],
    ),
    Section(
        title="Viewer Control",
        module_summary=None,
        func_list=["set_recording_id", "save"],
    ),
    Section(
        title="Time",
        module_summary=None,
        func_list=["set_time_sequence", "set_time_seconds", "set_time_nanos"],
    ),
    Section(
        title="Spatial Primitives",
        module_summary=None,
        func_list=[
            "log_point",
            "log_points",
            "log_rect",
            "log_rects",
            "log_obb",
            "log_line_strip",
            "log_line_segments",
            "log_arrow",
            "log_mesh",
            "log_meshes",
            "log_mesh_file",
        ],
    ),
    Section(
        title="Images",
        module_summary=None,
        func_list=["log_image", "log_image_file", "log_depth_image", "log_segmentation_image"],
    ),
    Section(
        title="Tensors",
        module_summary=None,
        func_list=["log_tensor"],
    ),
    Section(
        title="Annotations",
        module_summary=None,
        func_list=["log_annotation_context"],
    ),
    Section(
        title="Extension Components",
        module_summary=None,
        func_list=["log_extension_components"],
    ),
    Section(
        title="Plotting",
        module_summary=None,
        func_list=["log_scalar"],
    ),
    Section(
        title="Transforms",
        module_summary="log.transform",
        func_list=["log_rigid3", "log_pinhole", "log_unknown_transform", "log_view_coordinates"],
    ),
    Section(
        title="Text",
        module_summary=None,
        # TODO(#1251): Classes aren't supported yet
        # "LogLevel", "LoggingHandler"
        func_list=["log_text_entry"],
    ),
    Section(
        title="Helpers",
        module_summary="script_helpers",
        func_list=["script_add_args", "script_setup", "script_teardown"],
    ),
]

# Virtual folder where we will generate the md files
root = Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()
common_dir = Path("common")

# We use griffe to access docstrings
# Lots of other potentially interesting stuff we could pull out in the future
# This is what mkdocstrings uses under the hood
search_paths = [path for path in sys.path if path]  # eliminate empty path
search_paths.insert(0, root.as_posix())
rerun_pkg = griffe.load("depthai_viewer", search_paths=search_paths)

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
    # TODO(#1161): add links to our high-level docs!

    # Hide the TOC for the index since it's identical to the left nav-bar
    index_file.write(
        """---
hide:
    - toc
---
# Getting Started
* [Quick start](https://www.depthai_viewer.io/docs/getting-started/python)
* [Tutorial](https://www.depthai_viewer.io/docs/getting-started/logging-python)
* [Examples on GitHub](https://github.com/rerun-io/rerun/tree/latest/examples/python)
* [Troubleshooting](https://www.depthai_viewer.io/docs/getting-started/troubleshooting)

There are many different ways of sending data to the Depthai Viewer depending on what you're trying
to achieve and whether the viewer is running in the same process as your code, in another process,
or even as a separate web application.

Checkout [SDK Operating Modes](https://www.depthai_viewer.io/docs/reference/sdk-operating-modes) for an
overview of what's possible and how.

# APIs
"""
    )

    for section in SECTION_TABLE:
        # Turn the heading into a slug and add it to the nav
        md_name = make_slug(section.title)
        md_file = md_name + ".md"
        nav[section.title] = md_file

        # Write out the contents of this section
        write_path = common_dir.joinpath(md_file)
        with mkdocs_gen_files.open(write_path, "w") as fd:
            if section.module_summary is not None:
                fd.write(f"::: depthai_viewer.{section.module_summary}\n")
                fd.write("    options:\n")
                fd.write("      show_root_heading: False\n")
                fd.write("      members: []\n")
                fd.write("----\n")
            for func_name in section.func_list:
                fd.write(f"::: depthai_viewer.{func_name}\n")

        # Write out a table for the section in the index_file
        index_file.write(f"## {section.title}\n")
        index_file.write("Function | Description\n")
        index_file.write("-------- | -----------\n")
        for func_name in section.func_list:
            func = rerun_pkg[func_name]
            index_file.write(
                f"[`depthai_viewer.{func_name}()`]({md_name}#depthai_viewer.{func_name}) | {func.docstring.lines[0]}\n"
            )

        index_file.write("\n")

    index_file.write(
        """
# Troubleshooting
You can set `RUST_LOG=debug` before running your Python script
and/or `rerun` process to get some verbose logging output.

If you run into any issues don't hesitate to [open a ticket](https://github.com/rerun-io/rerun/issues/new/choose)
or [join our Discord](https://discord.gg/Gcm8BbTaAj).
"""
    )


# Generate the SUMMARY.txt file
with mkdocs_gen_files.open(common_dir.joinpath("SUMMARY.txt"), "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
