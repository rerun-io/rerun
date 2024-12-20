#!/usr/bin/env python3
"""
Generate an index table and rendered pages for the common APIs.

NOTE: When changing anything in this file, also consider how it affects `crates/build/re_dev_tools/src/build_search_index/ingest/python.rs`.

The top-level index file should look like
```
## Initialization
Function | Description
-------- | -----------
[rerun.init()](initialization/#rerun.init) | Initialize the Rerun SDK …
[rerun.connect_tcp()](initialization/#rerun.connect_tcp) | Connect to a remote Rerun Viewer on the …
[rerun.spawn()](initialization/#rerun.spawn) | Spawn a Rerun Viewer …
…

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


def all_archetypes() -> list[str]:
    file_path = Path(__file__).parent.parent.parent.joinpath("rerun_py/rerun_sdk/rerun/archetypes/__init__.py")

    # Initialize an empty list to store the quoted strings
    quoted_strings = []

    # Regular expression pattern to match quoted strings
    pattern = r'"([^"]*)"'

    # Open the file for reading
    with open(file_path, encoding="utf8") as file:
        # Read the file line by line
        for line in file:
            # Use re.findall to find all quoted strings in the line
            matches = re.findall(pattern, line)

            # Append the matched strings to the list
            quoted_strings.extend(matches)

    assert len(quoted_strings) > 0, f"Found no archetypes in {file_path}"
    return quoted_strings


@dataclass
class Section:
    title: str
    sub_title: str | None = None
    func_list: list[str] | None = None
    class_list: list[str] | None = None
    gen_page: bool = True
    mod_path: str = "rerun"
    show_tables: bool = True
    default_filters: bool = True
    show_submodules: bool = False


# This is the list of sections and functions that will be included in the index
# for each of them.
SECTION_TABLE: Final[list[Section]] = [
    ################################################################################
    Section(
        title="Initialization functions",
        func_list=[
            "init",
            "connect",
            "connect_tcp",
            "disconnect",
            "save",
            "send_blueprint",
            "serve",
            "serve_web",
            "spawn",
            "memory_recording",
            "notebook_show",
            "legacy_notebook_show",
        ],
    ),
    Section(
        title="Logging functions",
        func_list=[
            "log",
            "log_file_from_path",
            "log_file_from_contents",
        ],
    ),
    Section(
        title="Timeline functions",
        func_list=[
            "set_time_sequence",
            "set_time_seconds",
            "set_time_nanos",
            "disable_timeline",
            "reset_time",
        ],
    ),
    Section(
        title="Columnar API",
        func_list=[
            "send_columns",
        ],
        class_list=[
            "TimeNanosColumn",
            "TimeSecondsColumn",
            "TimeSequenceColumn",
        ],
    ),
    ################################################################################
    # These sections don't have tables, but generate pages containing all the archetypes, components, datatypes
    Section(
        title="Archetypes",
        mod_path="rerun.archetypes",
        show_tables=False,
    ),
    Section(
        title="Components",
        mod_path="rerun.components",
        show_tables=False,
    ),
    Section(
        title="Datatypes",
        mod_path="rerun.datatypes",
        show_tables=False,
    ),
    Section(
        title="Custom Data",
        class_list=[
            "AnyValues",
            "AnyBatchValue",
        ],
    ),
    ################################################################################
    # These are tables but don't need their own pages since they refer to types that
    # were added in the pages up above
    Section(
        title="Clearing Entities",
        class_list=["archetypes.Clear"],
        gen_page=False,
    ),
    Section(
        title="Annotations",
        class_list=[
            "archetypes.AnnotationContext",
            "datatypes.AnnotationInfo",
            "datatypes.ClassDescription",
        ],
        gen_page=False,
    ),
    Section(
        title="Images",
        class_list=[
            "archetypes.DepthImage",
            "archetypes.Image",
            "archetypes.EncodedImage",
            "archetypes.SegmentationImage",
        ],
        gen_page=False,
    ),
    Section(
        title="Video",
        class_list=[
            "archetypes.AssetVideo",
            "archetypes.VideoFrameReference",
        ],
        gen_page=False,
    ),
    Section(
        title="Plotting",
        class_list=[
            "archetypes.BarChart",
            "archetypes.Scalar",
            "archetypes.SeriesLine",
            "archetypes.SeriesPoint",
        ],
        gen_page=False,
    ),
    Section(
        title="Spatial Archetypes",
        class_list=[
            "archetypes.Arrows3D",
            "archetypes.Arrows2D",
            "archetypes.Asset3D",
            "archetypes.Boxes2D",
            "archetypes.Boxes3D",
            "archetypes.Capsules3D",
            "archetypes.Ellipsoids3D",
            "archetypes.LineStrips2D",
            "archetypes.LineStrips3D",
            "archetypes.Mesh3D",
            "archetypes.Points2D",
            "archetypes.Points3D",
        ],
        gen_page=False,
    ),
    Section(
        title="Geospatial Archetypes",
        class_list=[
            "archetypes.GeoLineStrings",
            "archetypes.GeoPoints",
        ],
        gen_page=False,
    ),
    Section(
        title="Graphs",
        class_list=[
            "archetypes.GraphNodes",
            "archetypes.GraphEdges",
        ],
        gen_page=False,
    ),
    Section(
        title="Tensors",
        class_list=["archetypes.Tensor"],
        gen_page=False,
    ),
    Section(
        title="Text",
        class_list=["LoggingHandler", "archetypes.TextDocument", "archetypes.TextLog"],
        gen_page=False,
    ),
    Section(
        title="Transforms and Coordinate Systems",
        class_list=[
            "archetypes.Pinhole",
            "archetypes.Transform3D",
            "archetypes.InstancePoses3D",
            "archetypes.ViewCoordinates",
            "components.Scale3D",
            "datatypes.Quaternion",
            "datatypes.RotationAxisAngle",
        ],
        gen_page=False,
    ),
    ################################################################################
    # Other referenced things
    Section(
        title="Enums",
        mod_path="rerun",
        class_list=[
            "Box2DFormat",
            "ImageFormat",
            "MeshFormat",
        ],
        show_tables=False,
    ),
    Section(
        title="Interfaces",
        mod_path="rerun",
        class_list=[
            "AsComponents",
            "ComponentBatchLike",
            "ComponentColumn",
        ],
        default_filters=False,
    ),
    ################################################################################
    # Blueprint APIs
    Section(
        title="Blueprint",
        sub_title="APIs",
        mod_path="rerun.blueprint",
        class_list=[
            "Blueprint",
            "BlueprintPart",
            "Container",
            "ContainerLike",
            "Horizontal",
            "Vertical",
            "Grid",
            "Tabs",
            "View",
            "BarChartView",
            "Spatial2DView",
            "Spatial3DView",
            "TensorView",
            "TextDocumentView",
            "TextLogView",
            "TimeSeriesView",
            "BlueprintPanel",
            "SelectionPanel",
            "TimePanel",
        ],
    ),
    Section(
        title="Blueprint",
        sub_title="Archetypes",
        mod_path="rerun.blueprint.archetypes",
        show_tables=False,
    ),
    Section(
        title="Blueprint",
        sub_title="Components",
        mod_path="rerun.blueprint.components",
        show_tables=False,
    ),
    Section(
        title="Blueprint",
        sub_title="Views",
        mod_path="rerun.blueprint.views",
        show_tables=False,
    ),
    ################################################################################
    # Remaining sections
    Section(
        title="Dataframe",
        mod_path="rerun.dataframe",
        func_list=[
            "load_archive",
            "load_recording",
        ],
        class_list=[
            "ComponentColumnDescriptor",
            "ComponentColumnSelector",
            "IndexColumnDescriptor",
            "IndexColumnSelector",
            "Recording",
            "RecordingView",
            "RRDArchive",
            "Schema",
            "AnyColumn",
            "AnyComponentColumn",
            "ComponentLike",
            "ViewContentsLike",
        ],
        show_tables=True,
    ),
    Section(
        title="Script Helpers",
        func_list=[
            "script_add_args",
            "script_setup",
            "script_teardown",
        ],
    ),
    Section(
        title="Other classes and functions",
        show_tables=False,
        func_list=[
            "get_data_recording",
            "get_global_data_recording",
            "get_recording_id",
            "get_thread_local_data_recording",
            "is_enabled",
            "new_recording",
            "set_global_data_recording",
            "set_thread_local_data_recording",
            "start_web_viewer_server",
            "escape_entity_path_part",
            "new_entity_path",
            "thread_local_stream",
            "recording_stream_generator_ctx",
        ],
        class_list=["RecordingStream", "LoggingHandler", "MemoryRecording"],
    ),
    Section(
        title="Utilities",
        show_tables=False,
        mod_path="rerun.utilities",
        show_submodules=True,
    ),
    # We don't have any experimental apis right now, but when you add one again, you should add this here:
    # Section(
    #     title="Experimental",
    #     func_list=[
    #         "my_experimental_function",
    #     ],
    #     show_tables=False,
    #     mod_path="rerun.experimental",
    # ),
]


def is_mentioned(thing: str) -> bool:
    for section in SECTION_TABLE:
        if section.class_list is not None:
            if f"archetypes.{thing}" in section.class_list:
                return True
    return False


# Virtual folder where we will generate the md files
rerun_py_root = Path(__file__).parent.parent.resolve()
sdk_root = Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()
common_dir = Path("common")

# Make sure all archetypes are included in the index:
for archetype in all_archetypes():
    assert is_mentioned(archetype), f"Archetype '{archetype}' is not mentioned in the index of {__file__}"

# We use griffe to access docstrings
# Lots of other potentially interesting stuff we could pull out in the future
# This is what mkdocstrings uses under the hood
search_paths = [path for path in sys.path if path]  # eliminate empty path

# This is where maturin puts rerun_bindings
search_paths.insert(0, rerun_py_root.as_posix())
# This is where the rerun package is
search_paths.insert(0, sdk_root.as_posix())

loader = griffe.GriffeLoader(search_paths=search_paths)

bindings_pkg = loader.load("rerun_bindings", find_stubs_package=True)
rerun_pkg = loader.load("rerun")

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
    index_file.write(
        """
## Getting Started
* [Quick start](https://www.rerun.io/docs/getting-started/quick-start/python)
* [Tutorial](https://www.rerun.io/docs/getting-started/data-in/python)
* [Examples on GitHub](https://github.com/rerun-io/rerun/tree/latest/examples/python)
* [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)

There are many different ways of sending data to the Rerun Viewer depending on what you're trying
to achieve and whether the viewer is running in the same process as your code, in another process,
or even as a separate web application.

Checkout [SDK Operating Modes](https://www.rerun.io/docs/reference/sdk/operating-modes) for an
overview of what's possible and how.

## APIs
"""
    )

    for section in SECTION_TABLE:
        if section.gen_page:
            # Turn the heading into a slug and add it to the nav
            if section.sub_title:
                md_name = make_slug("_".join([section.title, section.sub_title]))
                md_file = md_name + ".md"
                nav[(section.title, section.sub_title)] = md_file
            else:
                md_name = make_slug(section.title)
                md_file = md_name + ".md"
                nav[section.title] = md_file

            # Write out the contents of this section
            write_path = common_dir.joinpath(md_file)
            with mkdocs_gen_files.open(write_path, "w") as fd:
                fd.write(f"::: {section.mod_path}\n")
                fd.write("    options:\n")
                fd.write("      show_root_heading: True\n")
                fd.write("      heading_level: 3\n")
                fd.write("      members_order: alphabetical\n")
                # fd.write("      show_object_full_path: True\n")
                if section.func_list or section.class_list:
                    fd.write("      members:\n")
                    for func_name in section.func_list or []:
                        fd.write(f"        - {func_name}\n")
                    for class_name in section.class_list or []:
                        fd.write(f"        - {class_name}\n")
                if not section.default_filters:
                    fd.write("      filters: []\n")
                if section.show_submodules:
                    fd.write("      show_submodules: True\n")
            # Helpful for debugging
            if 0:
                with mkdocs_gen_files.open(write_path, "r") as fd:
                    print("FOR SECTION", section.title)
                    print(fd.read())
                    print()

        # Write out a table for the section in the index_file
        if section.show_tables:
            index_file.write(f"### {section.title}\n")
            if section.func_list:
                index_file.write("Function | Description\n")
                index_file.write("-------- | -----------\n")
                for func_name in section.func_list:
                    if section.mod_path != "rerun":
                        mod_tail = section.mod_path.split(".")[1:]
                        func_name = ".".join(mod_tail + [func_name])
                    func = rerun_pkg[func_name]
                    index_file.write(f"[`rerun.{func_name}()`][rerun.{func_name}] | {func.docstring.lines[0]}\n")
            if section.class_list:
                index_file.write("\n")
                index_file.write("Class | Description\n")
                index_file.write("-------- | -----------\n")
                for class_name in section.class_list:
                    if section.mod_path != "rerun":
                        mod_tail = section.mod_path.split(".")[1:]
                        class_name = ".".join(mod_tail + [class_name])
                    cls = rerun_pkg[class_name]
                    show_class = class_name
                    for maybe_strip in ["archetypes.", "components.", "datatypes."]:
                        if class_name.startswith(maybe_strip):
                            stripped = class_name.replace(maybe_strip, "")
                            if stripped in rerun_pkg.classes:
                                show_class = stripped
                    index_file.write(f"[`rerun.{show_class}`][rerun.{class_name}] | {cls.docstring.lines[0]}\n")

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
