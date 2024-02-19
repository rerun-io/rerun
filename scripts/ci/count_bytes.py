#!/usr/bin/env python3

"""
Measure sizes of a list of files.

This produces the format for use in https://github.com/benchmark-action/github-action-benchmark.

Use the script:
    python3 scripts/ci/count_bytes.py --help

    python3 scripts/ci/count_bytes.py \
        "Wasm":web_viewer/re_viewer_bg.wasm

    python3 scripts/ci/count_bytes.py --format=github \
        "Wasm":web_viewer/re_viewer_bg.wasm
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
    UNITS = ["B", "kiB", "MiB", "GiB", "TiB"]

    unit_index = 0
    while size > 1024:
        size /= 1024
        unit_index += 1

    return UNITS[unit_index]


DIVISORS = {
    "B": 1,
    "kiB": 1024,
    "MiB": 1024 * 1024,
    "GiB": 1024 * 1024 * 1024,
    "TiB": 1024 * 1024 * 1024 * 1024,
}


def get_divisor(unit: str) -> int:
    return DIVISORS[unit]


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


def compare(
    previous_path: str,
    current_path: str,
    threshold_pct: float,
    before_header: str,
    after_header: str,
) -> None:
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

    headers = ["Name", before_header, after_header, "Change"]
    rows: list[tuple[str, str, str, str]] = []
    for name, entry in entries.items():
        if "previous" in entry and "current" in entry:
            previous_bytes = float(entry["previous"]["value"]) * DIVISORS[entry["previous"]["unit"]]
            current_bytes = float(entry["current"]["value"]) * DIVISORS[entry["current"]["unit"]]
            unit = get_unit(min(previous_bytes, current_bytes))
            div = get_divisor(unit)

            abs_diff_bytes = abs(current_bytes - previous_bytes)
            min_diff_bytes = previous_bytes * (threshold_pct / 100)
            if abs_diff_bytes >= min_diff_bytes:
                previous = previous_bytes / div
                current = current_bytes / div
                change_pct = ((current_bytes - previous_bytes) / previous_bytes) * 100
                rows.append(
                    (
                        name,
                        f"{previous:.2f} {unit}",
                        f"{current:.2f} {unit}",
                        f"{change_pct:+.2f}%",
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
                "value": str(round(size / div, 2)),
                "unit": unit,
            }
        )

    sys.stdout.write(format.render(output))
    sys.stdout.flush()


def percentage(value: str) -> int:
    value = value.replace("%", "")
    return int(value)


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument(
        "--format",
        type=Format,
        choices=list(Format),
        default=Format.JSON,
        help="Format to render",
    )
    parser.add_argument("files", nargs="*", help="Entries to measure. Format: name:path[:unit]")

    args = parser.parse_args()
    measure(args.files, args.format)


if __name__ == "__main__":
    main()
