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
[rerun.connect_grpc()](initialization/#rerun.connect_grpc) | Connect to a remote Rerun Viewer on the …
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

# Modules we want public but get captured in other doc sections
EXCLUDE_SUBMODULE_CHECK = ["recording_stream", "sinks", "time", "web"]


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


def all_submodules(max_depth: int | None = None, ignore_hidden: bool = True) -> list[str]:
    """
    Walk the rerun package structure to find all submodules.

    Args:
        max_depth: Maximum depth to traverse. If None, traverse all levels.
                   Depth 1 would return 'blueprint' but not 'blueprint.archetypes'.
        ignore_hidden: If True, skip modules starting with underscores (e.g., _baseclasses, __main__).

    """

    rerun_package_path = Path(__file__).parent.parent.parent.joinpath("rerun_py/rerun_sdk/rerun")

    # Walk the filesystem directly instead of importing packages
    submodules = []

    # We do this because we build and test our docs without rerun_bindings built.
    def _walk_package_dir(directory: Path, base_path: Path, current_relative_path: str = "") -> None:
        """Recursively walk a Python package directory to find submodules."""
        if not directory.is_dir():
            return

        # Check if this directory is a Python package (has __init__.py)
        init_file = directory / "__init__.py"
        if not init_file.exists():
            return

        # If we're not at the root, add this as a submodule
        if current_relative_path:
            # Skip hidden modules if requested
            if ignore_hidden:
                parts = current_relative_path.split(".")
                if any(part.startswith("_") for part in parts):
                    return

            # Check depth if max_depth is specified
            if max_depth is not None:
                depth = current_relative_path.count(".") + 1
                if depth > max_depth:
                    return

            submodules.append(current_relative_path)

        # Recursively walk subdirectories
        for item in directory.iterdir():
            if item.is_dir() and not item.name.startswith("."):
                new_relative_path = current_relative_path + "." + item.name if current_relative_path else item.name
                _walk_package_dir(item, base_path, new_relative_path)

    _walk_package_dir(rerun_package_path, rerun_package_path)

    assert len(submodules) > 0, f"Found no submodules in {rerun_package_path}"
    return sorted(submodules)


@dataclass
class Section:
    title: str
    sub_title: str | None = None
    func_list: list[str] | None = None
    class_list: list[str] | None = None
    gen_page: bool = True
    mod_path: list[str] | None = None
    show_tables: bool = True
    default_filters: bool = True
    show_submodules: bool = False

    def __post_init__(self) -> None:
        if self.mod_path is None:
            self.mod_path = ["rerun"]


# This is the list of sections and functions that will be included in the index
# for each of them.
SECTION_TABLE: Final[list[Section]] = [
    ################################################################################
    Section(
        title="Initialization functions",
        func_list=[
            "init",
            "set_sinks",
            "connect_grpc",
            "disconnect",
            "save",
            "send_blueprint",
            "serve_grpc",
            "serve_web_viewer",
            "spawn",
            "memory_recording",
            "notebook_show",
            "legacy_notebook_show",
        ],
        class_list=["ChunkBatcherConfig", "DescribedComponentBatch", "RecordingStream", "TimeColumnLike"],
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
        title="Property functions",
        func_list=[
            "send_property",
            "send_recording_name",
            "send_recording_start_time_nanos",
        ],
    ),
    Section(
        title="Timeline functions",
        func_list=[
            "set_time",
            "disable_timeline",
            "reset_time",
        ],
    ),
    Section(
        title="Columnar API",
        func_list=[
            "send_columns",
            "send_record_batch",
            "send_dataframe",
        ],
        class_list=[
            "TimeColumn",
        ],
    ),
    ################################################################################
    # These sections don't have tables, but generate pages containing all the archetypes, components, datatypes
    Section(
        title="Archetypes",
        mod_path=["rerun.archetypes"],
        show_tables=False,
    ),
    Section(
        title="Components",
        mod_path=["rerun.components"],
        show_tables=False,
    ),
    Section(
        title="Datatypes",
        mod_path=["rerun.datatypes"],
        show_tables=False,
    ),
    Section(
        title="Custom Data",
        mod_path=["rerun.any_value", "rerun.any_batch_value", "rerun.dynamic_archetype"],
    ),
    ################################################################################
    # These are tables but don't need their own pages since they refer to types that
    # were added in the pages up above
    Section(
        title="General",
        class_list=[
            "archetypes.Clear",
            "blueprint.archetypes.EntityBehavior",
            "archetypes.RecordingInfo",
        ],
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
        title="ErrorUtils",
        mod_path=["rerun.error_utils"],
        show_tables=False,
    ),
    Section(
        title="Images",
        class_list=[
            "archetypes.DepthImage",
            "archetypes.Image",
            "archetypes.EncodedImage",
            "archetypes.EncodedDepthImage",
            "archetypes.SegmentationImage",
        ],
        gen_page=False,
    ),
    Section(
        title="Video",
        class_list=[
            "archetypes.VideoStream",
            "archetypes.AssetVideo",
            "archetypes.VideoFrameReference",
        ],
        gen_page=False,
    ),
    Section(
        title="Plotting",
        class_list=[
            "archetypes.BarChart",
            "archetypes.Scalars",
            "archetypes.SeriesLines",
            "archetypes.SeriesPoints",
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
            "archetypes.Cylinders3D",
            "archetypes.Ellipsoids3D",
            "archetypes.LineStrips2D",
            "archetypes.LineStrips3D",
            "archetypes.Mesh3D",
            "archetypes.Points2D",
            "archetypes.Points3D",
            "archetypes.TransformAxes3D",
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
            "archetypes.CoordinateFrame",
        ],
        gen_page=False,
    ),
    Section(
        title="MCAP",
        class_list=[
            "archetypes.McapChannel",
            "archetypes.McapMessage",
            "archetypes.McapSchema",
            "archetypes.McapStatistics",
        ],
        gen_page=False,
    ),
    # Section(
    #     title="Deprecated",
    #     class_list=[],
    #     gen_page=False,
    # ),
    ################################################################################
    # Other referenced things
    Section(
        title="Enums",
        mod_path=["rerun"],
        class_list=[
            "Box2DFormat",
            "ImageFormat",
            "MeshFormat",
        ],
        show_tables=False,
    ),
    Section(
        title="Interfaces",
        mod_path=["rerun"],
        class_list=[
            "ComponentMixin",
            "ComponentBatchLike",
            "AsComponents",
            "ComponentBatchLike",
            "ComponentColumn",
        ],
        default_filters=False,
        show_tables=True,
    ),
    ################################################################################
    # Blueprint APIs
    Section(
        title="Blueprint",
        sub_title="APIs",
        mod_path=["rerun.blueprint"],
        class_list=[
            "Blueprint",
            "BlueprintLike",
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
        mod_path=["rerun.blueprint.archetypes"],
        show_tables=False,
    ),
    Section(
        title="Blueprint",
        sub_title="Components",
        mod_path=["rerun.blueprint.components"],
        show_tables=False,
    ),
    Section(
        title="Blueprint",
        sub_title="Datatypes",
        mod_path=["rerun.blueprint.datatypes"],
        show_tables=False,
    ),
    Section(
        title="Blueprint",
        sub_title="Views",
        mod_path=["rerun.blueprint.views"],
        show_tables=False,
    ),
    ################################################################################
    # Remaining sections
    Section(
        title="Catalog",
        show_tables=True,
        mod_path=["rerun.catalog"],
        show_submodules=True,
        class_list=[
            "Schema",
            "ComponentColumnDescriptor",
            "ComponentColumnSelector",
            "IndexColumnDescriptor",
            "IndexColumnSelector",
            "AlreadyExistsError",
            "CatalogClient",
            "DatasetEntry",
            "DatasetView",
            "Entry",
            "EntryId",
            "EntryKind",
            "IndexValuesLike",
            "NotFoundError",
            "RegistrationHandle",
            "SegmentRegistrationResult",
            "TableEntry",
            "VectorDistanceMetric",
            "VectorDistanceMetricLike",
        ],
    ),
    Section(
        title="Server",
        show_tables=True,
        mod_path=["rerun.server"],
        show_submodules=True,
    ),
    Section(
        title="Authentication",
        show_tables=True,
        mod_path=["rerun.auth"],
        show_submodules=True,
    ),
    Section(
        title="Recording",
        mod_path=["rerun.recording"],
        func_list=[
            "load_archive",
            "load_recording",
        ],
        class_list=[
            "Recording",
            "RRDArchive",
        ],
        show_tables=True,
    ),
    Section(
        title="URDF Support",
        show_tables=True,
        mod_path=["rerun.urdf"],
        show_submodules=True,
    ),
    Section(
        title="Utilities",
        show_tables=False,
        mod_path=["rerun.utilities"],
        show_submodules=True,
    ),
    Section(
        title="Experimental",
        show_tables=True,
        mod_path=["rerun.experimental"],
        show_submodules=True,
    ),
    Section(
        title="Notebook",
        show_tables=True,
        mod_path=["rerun.notebook"],
        show_submodules=True,
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
        class_list=["LoggingHandler", "MemoryRecording", "GrpcSink", "FileSink"],
    ),
]


def is_archetype_mentioned(thing: str) -> bool:
    for section in SECTION_TABLE:
        if section.class_list is not None:
            if f"archetypes.{thing}" in section.class_list:
                return True
    return False


def is_submodule_mentioned(thing: str) -> bool:
    if thing in EXCLUDE_SUBMODULE_CHECK:
        return True
    for section in SECTION_TABLE:
        if section.mod_path is not None:
            for mod_path in section.mod_path:
                if thing == mod_path[len("rerun.") :]:
                    return True
    return False


# Virtual folder where we will generate the md files
rerun_py_root = Path(__file__).parent.parent.resolve()
sdk_root = Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()
common_dir = Path("common")

# Make sure all archetypes are included in the index:
for submodule in all_submodules(1, True):
    assert is_submodule_mentioned(submodule), (
        f"Submodule '{submodule}' is not mentioned in the index of {__file__};"
        " please add it to SECTION_TABLE for documentation, or prefix with underscore to hide it."
    )
for archetype in all_archetypes():
    assert is_archetype_mentioned(archetype), f"Archetype '{archetype}' is not mentioned in the index of {__file__}"

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

## Supported Python Versions

Rerun will typically support Python version up until their end-of-life. If you are using an older version
of Python, you can use the table below to make sure you choose the proper Rerun version for your Python installation.

| **Rerun Version** | **Release Date** | **Supported Python Version** |
|-------------------|------------------|------------------------------|
| 0.27              | Nov. 10, 2025    | 3.10+                        |
| 0.26              | Oct. 13, 2025    | 3.9+                         |
| 0.25              | Sep. 16, 2025    | 3.9+                         |
| 0.24              | Jul. 17, 2025    | 3.9+                         |
| 0.23              | Apr. 24, 2025    | 3.9+                         |
| 0.22              | Feb. 6, 2025     | 3.9+                         |
| 0.21              | Dec. 18. 2024    | 3.9+                         |
| 0.20              | Nov. 14, 2024    | 3.9+                         |
| 0.19              | Oct. 17, 2024    | 3.8+                         |


## APIs
""",
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
                for mod_path in section.mod_path:
                    fd.write(f"::: {mod_path}\n")
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
                    # Check if any mod_path is not "rerun" to determine formatting
                    non_rerun_paths = [path for path in section.mod_path if path != "rerun"]
                    if non_rerun_paths:
                        # Use the first non-rerun path for formatting
                        mod_tail = non_rerun_paths[0].split(".")[1:]
                        func_name = ".".join([*mod_tail, func_name])
                    func = rerun_pkg[func_name]
                    index_file.write(f"[`rerun.{func_name}()`][rerun.{func_name}] | {func.docstring.lines[0]}\n")
            if section.class_list:
                index_file.write("\n")
                index_file.write("Class | Description\n")
                index_file.write("-------- | -----------\n")
                for class_name in section.class_list:
                    # Check if any mod_path is not "rerun" to determine formatting
                    non_rerun_paths = [path for path in section.mod_path if path != "rerun"]
                    if non_rerun_paths:
                        # Use the first non-rerun path for formatting
                        mod_tail = non_rerun_paths[0].split(".")[1:]
                        class_name = ".".join([*mod_tail, class_name])
                    cls = rerun_pkg[class_name]
                    bindings_class = False
                    if "rerun_bindings" in cls.canonical_path:
                        bindings_class = True
                        # Get the docstring from the bindings package, but keep the rerun display path
                        cls = bindings_pkg[cls.canonical_path[len("rerun_bindings.") :]]
                        # Don't overwrite class_name - keep the rerun module path for display
                    show_class = class_name
                    for maybe_strip in ["archetypes.", "components.", "datatypes."]:
                        if class_name.startswith(maybe_strip):
                            stripped = class_name.replace(maybe_strip, "")
                            if stripped in rerun_pkg.classes:
                                show_class = stripped
                    # Always show as rerun.* in documentation, even for bindings classes
                    show_class = "rerun." + show_class
                    class_name = "rerun." + class_name
                    if cls.docstring is None:
                        raise ValueError(f"No docstring for class {class_name}")
                    index_file.write(f"[`{show_class}`][{class_name}] | {cls.docstring.lines[0]}\n")

        index_file.write("\n")

    index_file.write(
        """
# Troubleshooting
You can set `RUST_LOG=debug` before running your Python script
and/or `rerun` process to get some verbose logging output.

If you run into any issues don't hesitate to [open a ticket](https://github.com/rerun-io/rerun/issues/new/choose)
or [join our Discord](https://discord.gg/Gcm8BbTaAj).
""",
    )

# Generate the SUMMARY.txt file
with mkdocs_gen_files.open(common_dir.joinpath("SUMMARY.txt"), "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
