#!/usr/bin/env python3

"""
Compare sizes of a list of files.

This produces the format for use in <https://github.com/benchmark-action/github-action-benchmark>.

Use the script:
    python3 scripts/ci/compare.py --help

    python3 scripts/ci/compare.py --threshold=20 previous.json current.json
"""

from __future__ import annotations

import argparse
import json
import sys
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
    column_widths = [max(len(key), *(len(str(row[key])) for row in data)) for key in keys]
    separator = "|" + "|".join("-" * (width + 2) for width in column_widths)
    header_row = "|".join(f" {key.center(width)} " for key, width in zip(keys, column_widths, strict=False))

    table = f"|{header_row}|\n{separator}|\n"
    for row in data:
        row_str = "|".join(
            f" {str(row.get(key, '')).ljust(width)} " for key, width in zip(keys, column_widths, strict=False)
        )
        table += f"|{row_str}|\n"

    return table


def render_table_rows(rows: list[Any], headers: list[str]) -> str:
    column_widths = [max(len(str(item)) for item in col) for col in zip(*([tuple(headers), *rows]), strict=False)]
    separator = "|" + "|".join("-" * (width + 2) for width in column_widths)
    header_row = "|".join(f" {header.center(width)} " for header, width in zip(headers, column_widths, strict=False))

    table = f"|{header_row}|\n{separator}|\n"
    for row in rows:
        row_str = "|".join(f" {str(item).ljust(width)} " for item, width in zip(row, column_widths, strict=False))
        table += f"|{row_str}|\n"

    return table


def compare(
    previous_path: str,
    current_path: str,
    threshold_pct: float,
    before_header: str,
    after_header: str,
) -> None:
    previous = json.loads(Path(previous_path).read_text(encoding="utf-8"))
    current = json.loads(Path(current_path).read_text(encoding="utf-8"))

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
            previous_unit = entry["previous"]["unit"]
            current_unit = entry["current"]["unit"]

            previous = float(entry["previous"]["value"])
            current = float(entry["current"]["value"])

            if previous_unit == current_unit:
                div = 1
                unit = previous_unit
            else:
                previous_divisor = DIVISORS.get(previous_unit, 1)
                current_divisor = DIVISORS.get(current_unit, 1)

                previous_bytes = previous * previous_divisor
                current_bytes = current * current_divisor

                unit = get_unit(min(previous_bytes, current_bytes))
                div = get_divisor(unit)

                previous = previous_bytes / div
                current = current_bytes / div

            if previous == current:
                change_pct = 0.0  # e.g. both are zero
            elif previous == 0:
                change_pct = 100.0
            else:
                change_pct = 100 * (current - previous) / previous

            if abs(change_pct) >= threshold_pct:
                if unit in DIVISORS:
                    change = f"{change_pct:+.2f}%"
                else:
                    change = f"{format_num(current - previous)} {unit}"
                rows.append((
                    name,
                    f"{format_num(previous)} {unit}",
                    f"{format_num(current)} {unit}",
                    change,
                ))
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


def format_num(num: float) -> str:
    if num.is_integer():
        return str(int(num))
    return f"{num:.2f}"


def percentage(value: str) -> float:
    value = value.replace("%", "")
    return float(value)


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("before", type=str, help="Previous result .json file")
    parser.add_argument("after", type=str, help="Current result .json file")
    parser.add_argument(
        "--threshold",
        type=percentage,
        required=False,
        default=20,
        help="Only print row if value is N%% larger or smaller",
    )
    parser.add_argument(
        "--before-header",
        type=str,
        required=False,
        default="Before",
        help="Header for before column",
    )
    parser.add_argument(
        "--after-header",
        type=str,
        required=False,
        default="After",
        help="Header for after column",
    )

    args = parser.parse_args()

    compare(
        args.before,
        args.after,
        args.threshold,
        args.before_header,
        args.after_header,
    )


if __name__ == "__main__":
    main()
