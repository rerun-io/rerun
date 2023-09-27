#!/usr/bin/env python3
"""
Generates the permutations of view coordinate name definitions.

This will modify the different archetype extensions to include the appropriate constants.
"""
# TODO(#2388): This script can potentially go away or be significantly reduced.

from __future__ import annotations

import argparse
import itertools
import os
from dataclasses import dataclass
from typing import Iterable

BEGIN_MARKER = "<BEGIN_GENERATED:{}>"
END_MARKER = "<END_GENERATED:{}>"


SCRIPT_PATH = os.path.relpath(__file__, os.getcwd())

################################################################################


@dataclass
class ViewCoordinates:
    name: str
    x: str
    y: str
    z: str


def generate_view_permutations() -> Iterable[ViewCoordinates]:
    D1 = ["Up", "Down"]
    D2 = ["Left", "Right"]
    D3 = ["Forward", "Back"]
    for i in D1:
        for j in D2:
            for k in D3:
                for x, y, z in itertools.permutations([i, j, k]):
                    name = f"{x[0]}{y[0]}{z[0]}"
                    yield ViewCoordinates(name, x, y, z)


def generate_up_handed_permutations() -> Iterable[ViewCoordinates]:
    return [
        ViewCoordinates(name="RIGHT_HAND_X_UP", x="Up", y="Right", z="Forward"),
        ViewCoordinates(name="RIGHT_HAND_X_DOWN", x="Down", y="Right", z="Back"),
        ViewCoordinates(name="RIGHT_HAND_Y_UP", x="Right", y="Up", z="Back"),
        ViewCoordinates(name="RIGHT_HAND_Y_DOWN", x="Right", y="Down", z="Forward"),
        ViewCoordinates(name="RIGHT_HAND_Z_UP", x="Right", y="Forward", z="Up"),
        ViewCoordinates(name="RIGHT_HAND_Z_DOWN", x="Right", y="Back", z="Down"),
        ViewCoordinates(name="LEFT_HAND_X_UP", x="Up", y="Right", z="Back"),
        ViewCoordinates(name="LEFT_HAND_X_DOWN", x="Down", y="Right", z="Forward"),
        ViewCoordinates(name="LEFT_HAND_Y_UP", x="Right", y="Up", z="Forward"),
        ViewCoordinates(name="LEFT_HAND_Y_DOWN", x="Right", y="Down", z="Back"),
        ViewCoordinates(name="LEFT_HAND_Z_UP", x="Right", y="Back", z="Up"),
        ViewCoordinates(name="LEFT_HAND_Z_DOWN", x="Right", y="Forward", z="Down"),
    ]


################################################################################
# Rust Archetype
RUST_ARCHETYPE_EXTENSION_FILE = "crates/re_types/src/archetypes/view_coordinates_ext.rs"


def rust_arch_decl(coords: ViewCoordinates) -> str:
    return f"define_coordinates!({coords.name} => ({coords.x}, {coords.y}, {coords.z}));\n"


def gen_rust_arch_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --rust`\n")
    lines.extend(rust_arch_decl(v) for v in generate_view_permutations())
    lines.extend(rust_arch_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    return lines


################################################################################
# Rust Component
RUST_COMPONENT_EXTENSION_FILE = "crates/re_types/src/components/view_coordinates_ext.rs"


def rust_cmp_decl(coords: ViewCoordinates) -> str:
    return f"define_coordinates!({coords.name} => ({coords.x}, {coords.y}, {coords.z}));\n"


def gen_rust_cmp_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --rust`\n")
    lines.extend(rust_cmp_decl(v) for v in generate_view_permutations())
    lines.extend(rust_cmp_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    return lines


################################################################################
# Python Archetype

PYTHON_ARCHETYPE_EXTENSION_FILE = "rerun_py/rerun_sdk/rerun/archetypes/view_coordinates_ext.py"


def py_arch_decl(coords: ViewCoordinates) -> str:
    return f"{coords.name}: ViewCoordinates = None  # type: ignore[assignment]\n"


def gen_py_arch_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"# {BEGIN_MARKER.format(key)}\n")
    lines.append(f"# This section is generated by running `{SCRIPT_PATH} --python`\n")
    lines.append("# The following declarations are replaced in `deferred_patch_class`.\n")
    lines.extend(py_arch_decl(v) for v in generate_view_permutations())
    lines.extend(py_arch_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"# {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


def py_arch_def(coords: ViewCoordinates) -> str:
    return f"cls.{coords.name} = Component.{coords.name}\n"


def gen_py_arch_def() -> list[str]:
    key = "definitions"
    lines = []
    lines.append(f"# {BEGIN_MARKER.format(key)}\n")
    lines.append(f"# This section is generated by running `{SCRIPT_PATH} --python`\n")
    lines.extend(py_arch_def(v) for v in generate_view_permutations())
    lines.extend(py_arch_def(v) for v in generate_up_handed_permutations())
    lines.append(f"# {END_MARKER.format(key)}\n")
    lines = [" " * 8 + line for line in lines]
    return lines


################################################################################
# Python Component

PYTHON_COMPONENT_EXTENSION_FILE = "rerun_py/rerun_sdk/rerun/components/view_coordinates_ext.py"


def py_cmp_decl(coords: ViewCoordinates) -> str:
    return f"{coords.name}: ViewCoordinates = None  # type: ignore[assignment]\n"


def gen_py_cmp_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"# {BEGIN_MARKER.format(key)}\n")
    lines.append(f"# This section is generated by running `{SCRIPT_PATH} --python`\n")
    lines.append("# The following declarations are replaced in `deferred_patch_class`.\n")
    lines.extend(py_cmp_decl(v) for v in generate_view_permutations())
    lines.extend(py_cmp_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"# {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


def py_cmp_def(coords: ViewCoordinates) -> str:
    return f"cls.{coords.name} = cls([cls.ViewDir.{coords.x}, cls.ViewDir.{coords.y}, cls.ViewDir.{coords.z}])\n"


def gen_py_cmp_def() -> list[str]:
    key = "definitions"
    lines = []
    lines.append(f"# {BEGIN_MARKER.format(key)}\n")
    lines.append(f"# This section is generated by running `{SCRIPT_PATH} --python`\n")
    lines.extend(py_cmp_def(v) for v in generate_view_permutations())
    lines.extend(py_cmp_def(v) for v in generate_up_handed_permutations())
    lines.append(f"# {END_MARKER.format(key)}\n")
    lines = [" " * 8 + line for line in lines]
    return lines


################################################################################
# CPP Archetype
CPP_ARCHETYPE_EXTENSION_FILE = "rerun_cpp/src/rerun/archetypes/view_coordinates_ext.cpp"


def cpp_arch_decl(coords: ViewCoordinates) -> str:
    return f"static const rerun::archetypes::ViewCoordinates {coords.name};\n"


def gen_cpp_arch_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --cpp`\n")
    lines.extend(cpp_arch_decl(v) for v in generate_view_permutations())
    lines.extend(cpp_arch_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


def cpp_arch_def(coords: ViewCoordinates) -> str:
    return (
        f"const ViewCoordinates ViewCoordinates::{coords.name} = ViewCoordinates(\n"
        + f"rerun::components::ViewCoordinates::{coords.name}\n"
        + ");\n"
    )


def gen_cpp_arch_def() -> list[str]:
    key = "definitions"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --cpp`\n")
    lines.extend(cpp_arch_def(v) for v in generate_view_permutations())
    lines.extend(cpp_arch_def(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


################################################################################
# CPP Component
CPP_COMPONENT_EXTENSION_FILE = "rerun_cpp/src/rerun/components/view_coordinates_ext.cpp"


def cpp_cmp_decl(coords: ViewCoordinates) -> str:
    return f"static const rerun::components::ViewCoordinates {coords.name};\n"


def gen_cpp_cmp_decl() -> list[str]:
    key = "declarations"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --cpp`\n")
    lines.extend(cpp_cmp_decl(v) for v in generate_view_permutations())
    lines.extend(cpp_cmp_decl(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


def cpp_cmp_def(coords: ViewCoordinates) -> str:
    return (
        f"const ViewCoordinates ViewCoordinates::{coords.name} = ViewCoordinates(\n"
        + f"rerun::components::ViewCoordinates::{coords.x}, rerun::components::ViewCoordinates::{coords.y}, rerun::components::ViewCoordinates::{coords.z}\n"
        + ");\n"
    )


def gen_cpp_cmp_def() -> list[str]:
    key = "definitions"
    lines = []
    lines.append(f"// {BEGIN_MARKER.format(key)}\n")
    lines.append(f"// This section is generated by running `{SCRIPT_PATH} --cpp`\n")
    lines.extend(cpp_cmp_def(v) for v in generate_view_permutations())
    lines.extend(cpp_cmp_def(v) for v in generate_up_handed_permutations())
    lines.append(f"// {END_MARKER.format(key)}\n")
    lines = [" " * 4 + line for line in lines]
    return lines


################################################################################


def show_preview(lines: list[str]) -> None:
    print("".join(lines))


def patch_file(filename: str, lines: list[str], key: str) -> None:
    contents = open(filename).readlines()
    start_line = next((i for i, line in enumerate(contents) if BEGIN_MARKER.format(key) in line), None)
    end_line = next((i for i, line in enumerate(contents) if END_MARKER.format(key) in line), None)
    if (start_line is None) or (end_line is None):
        raise Exception("Could not find the generated section in the file.")
    new_contents = contents[:start_line] + lines + contents[end_line + 1 :]
    open(filename, "w").writelines(new_contents)


################################################################################


def process_file(preview: bool, filename: str, decl_lines: list[str] | None, def_lines: list[str] | None) -> None:
    if preview:
        if def_lines is not None:
            print(f"Preview of {filename}: definitions")
            show_preview(def_lines)
        if decl_lines is not None:
            print(f"Preview of {filename}: declarations")
            show_preview(decl_lines)
    else:
        if def_lines is not None:
            patch_file(filename, def_lines, "definitions")
        if decl_lines is not None:
            patch_file(filename, decl_lines, "declarations")


def main() -> None:
    parser = argparse.ArgumentParser(description="Modify the ViewCoordinate archetypes.")
    parser.add_argument(
        "--rust",
        action="store_true",
        default=False,
        help="Generate the rust code for the view coordinates.",
    )
    parser.add_argument(
        "--python",
        action="store_true",
        default=False,
        help="Generate the python code for the view coordinates.",
    )
    parser.add_argument(
        "--cpp",
        action="store_true",
        default=False,
        help="Generate the cpp code for the view coordinates.",
    )
    parser.add_argument(
        "--preview", action="store_true", default=False, help="Just print the preview of the generated sections"
    )
    args = parser.parse_args()

    if args.rust:
        process_file(args.preview, RUST_ARCHETYPE_EXTENSION_FILE, gen_rust_arch_decl(), None)
        process_file(args.preview, RUST_COMPONENT_EXTENSION_FILE, gen_rust_cmp_decl(), None)

    if args.python:
        process_file(args.preview, PYTHON_ARCHETYPE_EXTENSION_FILE, gen_py_arch_decl(), gen_py_arch_def())
        process_file(args.preview, PYTHON_COMPONENT_EXTENSION_FILE, gen_py_cmp_decl(), gen_py_cmp_def())

    if args.cpp:
        process_file(args.preview, CPP_ARCHETYPE_EXTENSION_FILE, gen_cpp_arch_decl(), gen_cpp_arch_def())
        process_file(args.preview, CPP_COMPONENT_EXTENSION_FILE, gen_cpp_cmp_decl(), gen_cpp_cmp_def())


if __name__ == "__main__":
    main()
