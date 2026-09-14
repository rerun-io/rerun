#!/usr/bin/env python3
"""Find puffin scopes whose name or data starts with a given prefix.

Reads JSON produced by `dump-puffin` and prints matching scopes, either as a
list of slowest individual instances or aggregated by `(name, data)`.

Usage:
    find_scopes.py <puffin.json> <prefix> [--field name|data|both]
                                          [--top N] [--aggregate]
                                          [--frame INDEX] [--thread NAME]

Examples:
    # all scopes whose name or data starts with "Redraw"
    find_scopes.py /tmp/puffin.json Redraw

    # only match the dynamic data field, top 50
    find_scopes.py /tmp/puffin.json query --field data --top 50

    # group by (name, data), see total time spent per group
    find_scopes.py /tmp/puffin.json scan --aggregate
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from collections.abc import Iterator
from typing import Any


def walk(node: dict[str, Any]) -> Iterator[dict[str, Any]]:
    """Yield every scope in a thread/frame subtree."""
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def matches(scope: dict[str, Any], prefix: str, field: str) -> bool:
    name = scope.get("name", "") or ""
    data = scope.get("data", "") or ""
    if field == "name":
        return name.startswith(prefix)
    if field == "data":
        return data.startswith(prefix)
    return name.startswith(prefix) or data.startswith(prefix)


def fmt_ms(ns: int) -> str:
    return f"{ns / 1e6:.3f} ms"


def main() -> None:
    p = argparse.ArgumentParser(description="Find puffin scopes by name/data prefix")
    p.add_argument("json_path", help="Path to dump-puffin JSON output")
    p.add_argument("prefix", help="Prefix to match against scope name and/or data")
    p.add_argument("--field", choices=["name", "data", "both"], default="both")
    p.add_argument("--top", type=int, default=20)
    p.add_argument("--aggregate", action="store_true", help="Group matches by (name, data) and sort by total time")
    p.add_argument("--frame", type=int, help="Restrict to a single frame_index")
    p.add_argument("--thread", help="Restrict to a single thread name")
    args = p.parse_args()

    with open(args.json_path) as f:
        data = json.load(f)

    matches_list: list[dict[str, Any]] = []
    for frame in data["frames"]:
        if args.frame is not None and frame["frame_index"] != args.frame:
            continue
        for thread in frame["threads"]:
            if args.thread is not None and thread["name"] != args.thread:
                continue
            for top in thread["scopes"]:
                for scope in walk(top):
                    if matches(scope, args.prefix, args.field):
                        scope = dict(scope)
                        scope["_frame"] = frame["frame_index"]
                        scope["_thread"] = thread["name"]
                        matches_list.append(scope)

    if not matches_list:
        print(f"No scopes matched prefix {args.prefix!r} (field={args.field}).")
        sys.exit(0)

    if args.aggregate:
        groups: dict[tuple[str, str], dict[str, Any]] = defaultdict(lambda: {"total_ns": 0, "count": 0, "max_ns": 0})
        for s in matches_list:
            key = (s.get("name", ""), s.get("data", ""))
            g = groups[key]
            g["total_ns"] += s["duration_ns"]
            g["count"] += 1
            g["max_ns"] = max(g["max_ns"], s["duration_ns"])

        agg_rows = sorted(groups.items(), key=lambda kv: -kv[1]["total_ns"])[: args.top]
        print(f"{'Total':>12}  {'Max':>10}  {'Count':>6}  name | data")
        print("-" * 70)
        for (name, dat), g in agg_rows:
            print(f"{fmt_ms(g['total_ns']):>12}  {fmt_ms(g['max_ns']):>10}  {g['count']:>6}  {name} | {dat}")
        return

    rows = sorted(matches_list, key=lambda s: -s["duration_ns"])[: args.top]
    print(f"{'Duration':>12}  frame  thread        name | data")  # NOLINT (column alignment)
    print("-" * 70)
    for s in rows:
        label = f"{s.get('name', '')} | {s.get('data', '')}"
        print(f"{fmt_ms(s['duration_ns']):>12}  {s['_frame']:>5}  {s['_thread']:<13} {label}")


if __name__ == "__main__":
    main()
