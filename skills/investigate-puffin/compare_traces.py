#!/usr/bin/env python3
"""Compare two puffin traces side by side, joined by (name, data).

Reads two JSON files produced by `dump-puffin` and prints a table where each row
is a `(name, data)` pair with totals, counts, and per-call costs from both
traces and their deltas. Sorted by largest absolute total-time change by
default, which surfaces regressions and improvements first.

Per-call cost (`total / count`) is the right number to look at when count is
unchanged — if a scope ran the same number of times but got slower per call,
something regressed inside it (e.g. lock contention, nested-pool starvation).

Usage:
    compare_traces.py <a.json> <b.json> [--top N] [--sort {delta,a,b,name}]
                                        [--filter PREFIX] [--field name|data|both]
                                        [--hide-below MS]

Examples:
    # default: top 20 rows by |Δtotal|
    compare_traces.py /tmp/puffin.json /tmp/puffin2.json

    # only rows whose name starts with "system_execution"
    compare_traces.py /tmp/puffin.json /tmp/puffin2.json --filter system_execution

    # ignore noise below 0.5 ms total in both traces
    compare_traces.py /tmp/puffin.json /tmp/puffin2.json --hide-below 0.5
"""

from __future__ import annotations

import argparse
import json
import operator
import sys
from collections import defaultdict
from collections.abc import Iterator
from typing import Any


def walk(node: dict[str, Any]) -> Iterator[dict[str, Any]]:
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def aggregate(path: str) -> tuple[dict[tuple[str, str], dict[str, int]], dict[str, Any]]:
    with open(path) as f:
        trace = json.load(f)

    groups: dict[tuple[str, str], dict[str, int]] = defaultdict(lambda: {"total_ns": 0, "count": 0})
    for frame in trace["frames"]:
        for thread in frame["threads"]:
            for top in thread["scopes"]:
                for scope in walk(top):
                    key = (scope.get("name", "") or "", scope.get("data", "") or "")
                    g = groups[key]
                    g["total_ns"] += scope.get("duration_ns", 0)
                    g["count"] += 1

    summary = {
        "frames": len(trace["frames"]),
        "total_ms": sum(f["duration_ns"] for f in trace["frames"]) / 1e6,
        "scopes_registered": len(trace.get("scopes", {})),
    }
    return groups, summary


def fmt_ms(ns: int) -> str:
    return f"{ns / 1e6:.2f}"


def fmt_signed_ms(delta_ns: int) -> str:
    sign = "+" if delta_ns >= 0 else "-"
    return f"{sign}{abs(delta_ns) / 1e6:.2f}"


def fmt_pct(a: float, b: float) -> str:
    if a == 0:
        return "  new" if b > 0 else "    -"
    if b == 0:
        return " gone"
    pct = (b - a) / a * 100
    sign = "+" if pct >= 0 else "-"
    return f"{sign}{abs(pct):>4.0f}%"


def matches_filter(name: str, data: str, prefix: str, field: str) -> bool:
    if field == "name":
        return name.startswith(prefix)
    if field == "data":
        return data.startswith(prefix)
    return name.startswith(prefix) or data.startswith(prefix)


def main() -> None:
    p = argparse.ArgumentParser(description="Compare two puffin traces by (name, data).")
    p.add_argument("a_path", help="First trace JSON (the 'before')")
    p.add_argument("b_path", help="Second trace JSON (the 'after')")
    p.add_argument("--top", type=int, default=20)
    p.add_argument(
        "--sort",
        choices=["delta", "a", "b", "name"],
        default="delta",
        help="Sort by |Δtotal| (default), A.total, B.total, or name",
    )
    p.add_argument("--filter", help="Only include rows whose name/data starts with this prefix")
    p.add_argument("--field", choices=["name", "data", "both"], default="both")
    p.add_argument(
        "--hide-below", type=float, default=0.0, help="Hide rows where both A.total and B.total are below this many ms"
    )
    args = p.parse_args()

    groups_a, summary_a = aggregate(args.a_path)
    groups_b, summary_b = aggregate(args.b_path)

    print(
        f"A: {args.a_path}  —  {summary_a['frames']} frames, "
        f"{summary_a['total_ms']:.2f} ms total"
        f"{'  (no scope registration)' if summary_a['scopes_registered'] == 0 else ''}"
    )
    print(
        f"B: {args.b_path}  —  {summary_b['frames']} frames, "
        f"{summary_b['total_ms']:.2f} ms total"
        f"{'  (no scope registration)' if summary_b['scopes_registered'] == 0 else ''}"
    )
    print()

    keys = set(groups_a) | set(groups_b)
    rows: list[dict[str, Any]] = []
    threshold_ns = int(args.hide_below * 1e6)
    for key in keys:
        a = groups_a.get(key, {"total_ns": 0, "count": 0})
        b = groups_b.get(key, {"total_ns": 0, "count": 0})
        if a["total_ns"] < threshold_ns and b["total_ns"] < threshold_ns:
            continue
        name, dat = key
        if args.filter and not matches_filter(name, dat, args.filter, args.field):
            continue
        rows.append({
            "name": name,
            "data": dat,
            "a_total": a["total_ns"],
            "b_total": b["total_ns"],
            "a_count": a["count"],
            "b_count": b["count"],
            "a_per": a["total_ns"] / a["count"] if a["count"] else 0,
            "b_per": b["total_ns"] / b["count"] if b["count"] else 0,
            "delta_total": b["total_ns"] - a["total_ns"],
        })

    if not rows:
        print("No rows after filtering.")
        sys.exit(0)

    if args.sort == "delta":
        rows.sort(key=lambda r: -abs(r["delta_total"]))
    elif args.sort == "a":
        rows.sort(key=lambda r: -r["a_total"])
    elif args.sort == "b":
        rows.sort(key=lambda r: -r["b_total"])
    else:
        rows.sort(key=operator.itemgetter("name", "data"))

    rows = rows[: args.top]

    header = (
        f"{'Δtotal':>9}  {'A.tot':>7}  {'B.tot':>7}  "
        f"{'A.cnt':>5} {'B.cnt':>5}  {'A.per':>7}  {'B.per':>7}  "
        f"{'Δper':>6}  name | data"
    )
    print(header)
    print("-" * len(header))
    for r in rows:
        label = f"{r['name']} | {r['data']}" if r["data"] else r["name"]
        print(
            f"{fmt_signed_ms(r['delta_total']):>9}  "
            f"{fmt_ms(r['a_total']):>7}  {fmt_ms(r['b_total']):>7}  "
            f"{r['a_count']:>5} {r['b_count']:>5}  "
            f"{fmt_ms(int(r['a_per'])):>7}  {fmt_ms(int(r['b_per'])):>7}  "
            f"{fmt_pct(r['a_per'], r['b_per']):>6}  {label}"
        )


if __name__ == "__main__":
    main()
