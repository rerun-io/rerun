#!/usr/bin/env python3

"""
Render benchmark graphs for the last 30 days.

Install dependencies:
    GitPython==3.1.37 google-cloud-storage==2.9.0

Use the script:
    python3 scripts/ci/render_bench.py
"""
from __future__ import annotations

import json
import os
import re
from datetime import datetime, timedelta
from subprocess import run
from typing import Any, Dict, Generator, List

from google.cloud import storage

SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))


def non_empty_lines(s: str) -> Generator[str, None]:
    for line in s.splitlines():
        if len(line.strip()) == 0:
            continue
        yield line


def git_log(date: datetime) -> list[str] | None:
    args = ["git", "log"]
    args += [f'--after="{date.year}-{date.month}-{date.day} 00:00:00"']
    args += [f'--before="{date.year}-{date.month}-{date.day} 23:59:59"']
    args += ["--format=%H"]
    commits = run(args, check=True, capture_output=True, text=True).stdout.strip()
    if len(commits) == 0:
        return None
    else:
        return commits.strip().splitlines()


CommitsByDate = Dict[datetime, List[str]]


def get_commits(end_date: datetime, num_days: int) -> CommitsByDate:
    """Yields the list of commits for each day between `end_date - num_days` and `end_date`."""

    start = (end_date - timedelta(days=num_days)).strftime("%Y-%m-%d")
    end = (end_date).strftime("%Y-%m-%d")
    print(f"Fetching commits between {start} and {end}")

    commits_by_date = {}
    previous_log = None

    for offset in reversed(range(num_days)):
        date = end_date - timedelta(days=offset)
        log = git_log(date) or previous_log
        if log is None:
            continue
        commits_by_date[date] = log
        previous_log = log

    return commits_by_date


BenchmarkEntry = Dict[str, Any]
Benchmarks = Dict[str, List[BenchmarkEntry]]


FORMAT_BENCHER_RE = re.compile(r"test\s+(\S+).*bench:\s+(\d+)\s+ns\/iter")


def parse_bencher_line(data: str) -> BenchmarkEntry:
    name, ns_iter = FORMAT_BENCHER_RE.match(data).groups()
    return {"name": name, "value": float(ns_iter), "unit": "ns/iter"}


def parse_sizes_json(data: str) -> list[BenchmarkEntry]:
    out = []
    for entry in json.loads(data):
        out.append({"name": entry["name"], "value": float(entry["value"]), "unit": entry["unit"]})
    return out


def get_runtime_benchmarks(gcs: storage.Client, commits_by_date: CommitsByDate) -> Benchmarks:
    print("Fetching runtime benchmarks…")

    benchmarks: Benchmarks = {}
    bucket = gcs.bucket("rerun-builds")
    for date, commits in commits_by_date.items():
        for commit in commits:
            short_sha = commit[0:7]
            blob = bucket.get_blob(f"benches/{short_sha}")
            if blob is None:
                continue
            data = blob.download_as_text()
            for line in non_empty_lines(data):
                entry = parse_bencher_line(line)
                name = entry["name"]
                entry["date"] = f"{date.year}-{date.month}-{date.day}"
                entry["commit"] = short_sha
                if name not in benchmarks:
                    benchmarks[name] = []
                benchmarks[name].append(entry)
            break

    return benchmarks


def get_size_benchmarks(gcs: storage.Client, commits_by_date: CommitsByDate) -> Benchmarks:
    print("Fetching size benchmarks…")

    benchmarks: Benchmarks = {}
    bucket = gcs.bucket("rerun-builds")
    for date, commits in commits_by_date.items():
        for commit in commits:
            short_sha = commit[0:7]
            blob = bucket.get_blob(f"sizes/commit/{short_sha}/data.json")
            if blob is None:
                continue
            for entry in parse_sizes_json(blob.download_as_text()):
                name = entry["name"]
                entry["date"] = f"{date.year}-{date.month}-{date.day}"
                entry["commit"] = short_sha
                if name not in benchmarks:
                    benchmarks[name] = []
                benchmarks[name].append(entry)
            break
    return benchmarks


BYTE_UNITS = ["b", "kb", "kib", "mb", "mib", "gb", "gib", "tb", "tib"]
VALID_CONVERSIONS = {
    "ns/iter": ["ns/iter"],
}
for unit in BYTE_UNITS:
    VALID_CONVERSIONS[unit] = BYTE_UNITS

UNITS = {
    "b": 1,
    "kb": 1000,
    "kib": 1024,
    "mb": 1000,
    "mib": 1024 * 1024,
    "gb": 1000,
    "gib": 1024 * 1024,
    "tb": 1000,
    "tib": 1024 * 1024,
    "ns/iter": 1,
}


def normalize(base_unit: str, unit: str, value: float) -> float:
    """Convert `value` from `base_unit` to `unit`."""
    base_unit = base_unit.lower()
    unit = unit.lower()
    if unit not in VALID_CONVERSIONS[base_unit]:
        raise Exception(f"invalid conversion from {base_unit} to {unit}")
    return value / UNITS[unit] * UNITS[base_unit]


def render_html(title: str, benchmarks: Benchmarks) -> str:
    print(f"Rendering {title}…")

    chartjs = {}
    for name, benchmark in benchmarks.items():
        if len(benchmark) == 0:
            chartjs[name] = None
        labels = [entry["date"] for entry in benchmark]
        base_unit = benchmark[0]["unit"]
        data = [normalize(base_unit, entry["unit"], entry["value"]) for entry in benchmark]
        chartjs[name] = {
            "type": "line",
            "data": {
                "labels": labels,
                "datasets": [
                    {
                        "label": name,
                        "data": data,
                        "borderColor": "#dea584",
                        "backgroundColor": "#dea58460",
                        "fill": True,
                    }
                ],
            },
            "options": {
                "scales": {
                    "x": {"beginAtZero": True},
                    "y": {"beginAtZero": True, "title": base_unit},
                }
            },
        }

    with open(os.path.join(SCRIPT_PATH, "templates/benchmark.html")) as template_file:
        html = template_file.read()
        html = html.replace("%%TITLE%%", title)
        # double encode to escape the data as a single string
        html = html.replace('"%%CHARTS%%"', json.dumps(json.dumps(chartjs)))
    return html


def main() -> None:
    gcs = storage.Client()

    runtime = get_runtime_benchmarks(gcs, get_commits(datetime.today(), 30))
    with open(os.path.join(SCRIPT_PATH, "runtime.html"), "w") as f:
        f.write(render_html("runtime benchmarks", runtime))

    sizes = get_size_benchmarks(gcs, get_commits(datetime.today(), 30 * 6))
    with open(os.path.join(SCRIPT_PATH, "benches.html"), "w") as f:
        f.write(render_html("size benchmarks", sizes))

    # TODO: upload to gcs
    #       use on CI instead of github-action-benchmark


if __name__ == "__main__":
    main()
