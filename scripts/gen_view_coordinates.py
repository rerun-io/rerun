#!/usr/bin/env python3
"""
Generates the permutations of view coordinate names.

This will modify the different archetype extensions to include the appropriate constants.
"""

import argparse
import itertools
from typing import Iterable

from attr import dataclass

RUST_EXTENSION = "crates/re_types/src/archetypes/view_coordinates_ext.rs"


@dataclass
class ViewCoordinates:
    name: str
    axis0: str
    axis1: str
    axis2: str


def generate_view_permutations() -> Iterable[ViewCoordinates]:
    D1 = ["Up", "Down"]
    D2 = ["Left", "Right"]
    D3 = ["Forward", "Back"]
    for x in D1:
        for y in D2:
            for z in D3:
                for p0, p1, p2 in itertools.permutations([x, y, z]):
                    name = f"{p0[0]}{p1[0]}{p2[0]}"
                    yield ViewCoordinates(name, p0, p1, p2)


def generate_up_handed_permuations() -> Iterable[ViewCoordinates]:
    return [
        ViewCoordinates(name="RIGHT_HAND_POS_X_UP", axis0="Up", axis1="Right", axis2="Forward"),
        ViewCoordinates(name="RIGHT_HAND_NEG_X_UP", axis0="Down", axis1="Right", axis2="Back"),
        ViewCoordinates(name="RIGHT_HAND_POS_Y_UP", axis0="Right", axis1="Up", axis2="Back"),
        ViewCoordinates(name="RIGHT_HAND_NEG_Y_UP", axis0="Right", axis1="Down", axis2="Forward"),
        ViewCoordinates(name="RIGHT_HAND_POS_Z_UP", axis0="Right", axis1="Forward", axis2="Up"),
        ViewCoordinates(name="RIGHT_HAND_NEG_Z_UP", axis0="Right", axis1="Back", axis2="Down"),
        ViewCoordinates(name="LEFT_HAND_POS_X_UP", axis0="Up", axis1="Right", axis2="Back"),
        ViewCoordinates(name="LEFT_HAND_NEG_X_UP", axis0="Down", axis1="Right", axis2="Forward"),
        ViewCoordinates(name="LEFT_HAND_POS_Y_UP", axis0="Right", axis1="Up", axis2="Forward"),
        ViewCoordinates(name="LEFT_HAND_NEG_Y_UP", axis0="Right", axis1="Down", axis2="Back"),
        ViewCoordinates(name="LEFT_HAND_POS_Z_UP", axis0="Right", axis1="Back", axis2="Up"),
        ViewCoordinates(name="LEFT_HAND_NEG_Z_UP", axis0="Right", axis1="Forward", axis2="Down"),
    ]


def rust_definition(coords: ViewCoordinates) -> str:
    return f"define_coordinates!({coords.name} => ({coords.axis0}, {coords.axis1}, {coords.axis2}));\n"


def gen_rust_code() -> list[str]:
    lines = []
    lines.append("// <BEGIN_GENERATED>\n")
    lines.append("// This section is generated by running `scripts/gen_view_coordinates.py --rust`\n")
    lines.extend(rust_definition(v) for v in generate_view_permutations())
    lines.extend(rust_definition(v) for v in generate_up_handed_permuations())
    lines.append("// <END_GENERATED>\n")
    return lines


def patch_file(filename: str, lines: list[str]) -> None:
    contents = open(filename).readlines()
    start_line = next((i for i, line in enumerate(contents) if "<BEGIN_GENERATED>" in line), None)
    end_line = next((i for i, line in enumerate(contents) if "<END_GENERATED>" in line), None)
    if (start_line is None) or (end_line is None):
        raise Exception("Could not find the generated section in the file.")
    new_contents = contents[:start_line] + lines + contents[end_line + 1 :]
    open(filename, "w").writelines(new_contents)


def main() -> None:
    parser = argparse.ArgumentParser(description="Modify the ViewCoordinate archetypes.")
    parser.add_argument(
        "--rust",
        action="store_true",
        default=False,
        help="Generate the rust code for the view coordinates.",
    )
    parser.add_argument(
        "--preview", action="store_true", default=False, help="Just print the preview of the generated sections"
    )
    args = parser.parse_args()

    if args.rust:
        if args.preview:
            print(gen_rust_code())
        else:
            patch_file(RUST_EXTENSION, gen_rust_code())


if __name__ == "__main__":
    main()
