#!/usr/bin/env python3

"""
Measure or compare sizes of a list of files.

This produces the format for use in https://github.com/benchmark-action/github-action-benchmark.

Use the script:
    python3 scripts/ci/sizes.py --help

    python3 scripts/ci/sizes.py measure \
        "Wasm (release)":web_viewer/re_viewer_bg.wasm \
        "Wasm (debug)":web_viewer/re_viewer_debug_bg.wasm

    python3 scripts/ci/sizes.py measure --format=table \
        "Wasm (release)":web_viewer/re_viewer_bg.wasm \
        "Wasm (debug)":web_viewer/re_viewer_debug_bg.wasm

    python3 scripts/ci/sizes.py compare --threshold=20 previous.json current.json
"""
from __future__ import annotations

import argparse
import json
import os.path
import sys
from enum import Enum
from pathlib import Path
from typing import Any


def get_unit(size: int | float) -> str:
    UNITS = ["B", "KB", "MB", "GB", "TB"]

    unit_index = 0
    while size > 1024:
        size /= 1024
        unit_index += 1

    return UNITS[unit_index]


DIVISORS = {
    "B": 1,
    "KB": 1024,
    "MB": 1024 * 1024,
    "GB": 1024 * 1024 * 1024,
    "TB": 1024 * 1024 * 1024 * 1024,
}


def get_divisor(unit: str) -> int:
    return DIVISORS[unit.upper()] or 1


def cell(value: float, div: float) -> str:
    return str(round(value / div, 3))


def render_table_dict(data: list[dict[str, str]]) -> str:
    keys = data[0].keys()
    column_widths = [max(len(key), max(len(str(row[key])) for row in data)) for key in keys]
    separator = "|" + "|".join("-" * (width + 2) for width in column_widths)
    header_row = "|".join(f" {key.center(width)} " for key, width in zip(keys, column_widths))

    table = f"|{header_row}|\n{separator}|\n"
    for row in data:
        row_str = "|".join(f" {str(row.get(key, '')).ljust(width)} " for key, width in zip(keys, column_widths))
        table += f"|{row_str}|\n"

    return table


def render_table_rows(rows: list[Any], headers: list[str]) -> str:
    column_widths = [max(len(str(item)) for item in col) for col in zip(*([tuple(headers)] + rows))]
    separator = "|" + "|".join("-" * (width + 2) for width in column_widths)
    header_row = "|".join(f" {header.center(width)} " for header, width in zip(headers, column_widths))

    table = f"|{header_row}|\n{separator}|\n"
    for row in rows:
        row_str = "|".join(f" {str(item).ljust(width)} " for item, width in zip(row, column_widths))
        table += f"|{row_str}|\n"

    return table


class Format(Enum):
    JSON = "json"
    GITHUB = "github"

    def render(self, data: list[dict[str, str]]) -> str:
        if self is Format.JSON:
            return json.dumps(data)
        if self is Format.GITHUB:
            return render_table_dict(data)


def compare(previous_path: str, current_path: str, threshold: float) -> None:
    previous = json.loads(Path(previous_path).read_text())
    current = json.loads(Path(current_path).read_text())

    entries = {}
    for entry in current:
        name = entry["name"]
        entries[name] = {"current": entry}
    for entry in previous:
        name = entry["name"]
        if name not in entries:
            entries[name] = {}
        entries[name]["previous"] = entry

    headers = ["Name", "Previous", "Current", "Change"]
    rows: list[tuple[str, str, str, str]] = []
    for name, entry in entries.items():
        if "previous" in entry and "current" in entry:
            previous = float(entry["previous"]["value"]) * DIVISORS[entry["previous"]["unit"]]
            current = float(entry["current"]["value"]) * DIVISORS[entry["current"]["unit"]]

            min_change = previous * (threshold / 100)

            unit = get_unit(min(previous, current))
            div = get_divisor(unit)

            change = ((previous / current) * 100) - 100
            sign = "+" if change > 0 else ""

            if abs(current - previous) >= min_change:
                rows.append(
                    (
                        name,
                        f"{cell(previous, div)} {unit}",
                        f"{cell(current, div)} {unit}",
                        sign + str(change) + "%",
                    )
                )
        elif "current" in entry:
            value = entry["current"]["value"]
            unit = entry["current"]["unit"]

            rows.append((name, "(none)", f"{value} {unit}", "+100%"))
        elif "previous" in entry:
            value = entry["previous"]["value"]
            unit = entry["previous"]["unit"]

            rows.append((name, f"{value} {unit}", "(deleted)", "-100%"))

    if len(rows) > 0:
        sys.stdout.write(render_table_rows(rows, headers))
        sys.stdout.flush()


def measure(files: list[str], format: Format) -> None:
    output: list[dict[str, str]] = []
    for arg in files:
        parts = arg.split(":")
        name = parts[0]
        file = parts[1]
        size = os.path.getsize(file)
        unit = parts[2] if len(parts) > 2 else get_unit(size)
        div = get_divisor(unit)

        output.append(
            {
                "name": name,
                "value": str(round(size / div, 3)),
                "unit": unit,
            }
        )

    sys.stdout.write(format.render(output))
    sys.stdout.flush()


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")

    cmds_parser = parser.add_subparsers(title="cmds", dest="cmd", help="Command")

    compare_parser = cmds_parser.add_parser("compare", help="Compare results")
    compare_parser.add_argument("before", type=str, help="Previous result .json file")
    compare_parser.add_argument("after", type=str, help="Current result .json file")
    compare_parser.add_argument(
        "--threshold",
        type=float,
        required=False,
        default=20,
        help="Only print row if value is N%% larger or smaller",
    )

    measure_parser = cmds_parser.add_parser("measure", help="Measure sizes")
    measure_parser.add_argument(
        "--format",
        type=Format,
        choices=list(Format),
        default=Format.JSON,
        help="Format to render",
    )
    measure_parser.add_argument("files", nargs="*", help="Entries to measure. Format: name:path[:unit]")

    args = parser.parse_args()

    if args.cmd == "compare":
        compare(args.before, args.after, args.threshold)
    elif args.cmd == "measure":
        measure(args.files, args.format)


if __name__ == "__main__":
    main()
