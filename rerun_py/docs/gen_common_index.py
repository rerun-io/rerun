r"""
Generate an index table and rendered pages for the common APIs.

The top-level index file should look like
```
## Initialization
Function | Description
-------- | -----------
[rerun.init()](initialization/#rerun.init) | Initialize the Rerun SDK ...
[rerun.connect()](initialization/#rerun.connect) | Connect to a remote Rerun Viewer on the ...
[rerun.spawn()](initialization/#rerun.spawn) | Spawn a Rerun Viewer ...
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
from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Final

import griffe
import mkdocs_gen_files


@dataclass
class Section:
    title: str
    module_summary: str | None
    func_list: list[str]
    class_list: list[str]


# This is the list of sections and functions that will be included in the index
# for each of them.
SECTION_TABLE: Final[list[Section]] = [
    Section(
        title="Initialization",
        module_summary=None,
        func_list=["init", "connect", "disconnect", "spawn", "serve", "save", "memory_recording"],
        class_list=[],
    ),
    Section(
        title="Viewer Control",
        module_summary=None,
        func_list=["save"],
        class_list=[],
    ),
    Section(
        title="Time",
        module_summary=None,
        func_list=["set_time_sequence", "set_time_seconds", "set_time_nanos"],
        class_list=[],
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
        class_list=[],
    ),
    Section(
        title="Images",
        module_summary=None,
        func_list=["log_image", "log_image_file", "log_depth_image", "log_segmentation_image"],
        class_list=[],
    ),
    Section(
        title="Tensors",
        module_summary=None,
        func_list=["log_tensor"],
        class_list=[],
    ),
    Section(
        title="Annotations",
        module_summary=None,
        func_list=["log_annotation_context"],
        class_list=["ClassDescription", "AnnotationInfo"],
    ),
    Section(
        title="Extension Components",
        module_summary=None,
        func_list=["log_extension_components"],
        class_list=[],
    ),
    Section(
        title="Plotting",
        module_summary=None,
        func_list=["log_scalar"],
        class_list=[],
    ),
    Section(
        title="Transforms",
        module_summary="log.transform",
        func_list=[
            "log_transform3d",
            "log_pinhole",
            "log_unknown_transform",
            "log_disconnected_space",
            "log_view_coordinates",
            "log_rigid3",
        ],
        class_list=[],
    ),
    Section(
        title="Text",
        module_summary=None,
        # TODO(#1251): Classes aren't supported yet
        # "LogLevel", "LoggingHandler"
        func_list=["log_text_entry"],
        class_list=["LogLevel", "LoggingHandler"],
    ),
    Section(
        title="Clearing Entities",
        module_summary=None,
        func_list=["log_cleared"],
        class_list=[],
    ),
    Section(
        title="Helpers",
        module_summary="script_helpers",
        func_list=["script_add_args", "script_setup", "script_teardown"],
        class_list=[],
    ),
    Section(
        title="Experimental",
        module_summary="experimental",
        func_list=[
            "experimental.log_text_box",
            "experimental.new_blueprint",
            "experimental.add_space_view",
            "experimental.set_panels",
            "experimental.set_auto_space_views",
        ],
        class_list=[],
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
    # TODO(#1161): add links to our high-level docs!

    # Hide the TOC for the index since it's identical to the left nav-bar
    index_file.write(
        """---
hide:
    - toc
---
# Getting Started
* [Quick start](https://www.rerun.io/docs/getting-started/python)
* [Tutorial](https://www.rerun.io/docs/getting-started/logging-python)
* [Examples on GitHub](https://github.com/rerun-io/rerun/tree/latest/examples/python)
* [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)

There are many different ways of sending data to the Rerun Viewer depending on what you're trying
to achieve and whether the viewer is running in the same process as your code, in another process,
or even as a separate web application.

Checkout [SDK Operating Modes](https://www.rerun.io/docs/reference/sdk-operating-modes) for an
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
                fd.write(f"::: rerun.{section.module_summary}\n")
                fd.write("    options:\n")
                fd.write("      show_root_heading: False\n")
                fd.write("      members: []\n")
                fd.write("----\n")
            for func_name in section.func_list:
                fd.write(f"::: rerun.{func_name}\n")
                fd.write("    options:\n")
                fd.write("      heading_level: 4\n")
            for class_name in section.class_list:
                # fd.write(f"::: rerun.{class_name}\n")
                fd.write(f"::: rerun.{class_name}\n")
                fd.write("    options:\n")
                fd.write("      heading_level: 4\n")

        # Write out a table for the section in the index_file
        index_file.write(f"## {section.title}\n")
        if section.func_list:
            index_file.write("Function | Description\n")
            index_file.write("-------- | -----------\n")
            for func_name in section.func_list:
                func = rerun_pkg[func_name]
                index_file.write(f"[`rerun.{func_name}()`]({md_name}#rerun.{func_name}) | {func.docstring.lines[0]}\n")
        if section.class_list:
            index_file.write("\n")
            index_file.write("Class | Description\n")
            index_file.write("-------- | -----------\n")
            for class_name in section.class_list:
                cls = rerun_pkg[class_name]
                index_file.write(f"[`rerun.{class_name}`]({md_name}#rerun.{class_name}) | {cls.docstring.lines[0]}\n")

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
