#!/usr/bin/env python3

"""
Render benchmark graphs from data in GCS.

To use this script, you must be authenticated with GCS,
see https://cloud.google.com/docs/authentication/client-libraries for more information.

Install dependencies:
    GitPython==3.1.37 google-cloud-storage==2.9.0

Use the script:
    python3 scripts/ci/render_bench.py
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import textwrap
from datetime import datetime, timedelta
from enum import Enum
from subprocess import run
from typing import Any, Callable, Dict, Generator, List

from google.cloud import storage

SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))
DATE_FORMAT = "%Y-%m-%d"
ESCAPED_DATE_FORMAT = DATE_FORMAT.replace("%", "%%")


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


BucketPrefix = Dict[str, storage.Blob]


def fetch_bucket(gcs: storage.Client, name: str, path_prefix: str) -> BucketPrefix:
    bucket = gcs.bucket(name)
    blobs = bucket.list_blobs(prefix=path_prefix)
    return {blob.name: blob for blob in blobs}


def collect_benchmark_data(
    commits_by_date: CommitsByDate,
    bucket: BucketPrefix,
    short_sha_to_path: Callable[[str], str],
    entry_parser: Callable[[str], list[BenchmarkEntry]],
) -> Benchmarks:
    benchmarks: Benchmarks = {}

    def insert(name: str, entry: BenchmarkEntry) -> None:
        if name not in benchmarks:
            benchmarks[name] = []
        benchmarks[name].append(entry)

    for date, commits in commits_by_date.items():
        for commit in commits:
            short_sha = commit[0:7]
            path = short_sha_to_path(short_sha)
            if path not in bucket:
                continue
            for entry in entry_parser(bucket[path].download_as_text()):
                name = entry["name"]
                entry["date"] = f"{date.year}-{date.month}-{date.day}"
                entry["commit"] = short_sha
                insert(name, entry)
            break

    return benchmarks


def get_crates_benchmark_data(gcs: storage.Client, commits_by_date: CommitsByDate) -> Benchmarks:
    print('Fetching benchmark data for "Rust Crates"…')

    return collect_benchmark_data(
        commits_by_date,
        bucket=fetch_bucket(gcs, "rerun-builds", "benches"),
        short_sha_to_path=lambda short_sha: f"benches/{short_sha}",
        entry_parser=lambda data: [parse_bencher_line(line) for line in non_empty_lines(data)],
    )


def get_size_benchmark_data(gcs: storage.Client, commits_by_date: CommitsByDate) -> Benchmarks:
    print('Fetching benchmark data for "Sizes"…')

    return collect_benchmark_data(
        commits_by_date,
        bucket=fetch_bucket(gcs, "rerun-builds", "sizes/commit"),
        short_sha_to_path=lambda short_sha: f"sizes/commit/{short_sha}/data.json",
        entry_parser=parse_sizes_json,
    )


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


def min_and_max(data: list[float]) -> (float, float):
    min_value = float("inf")
    max_value = float("-inf")
    for value in data:
        if value < min_value:
            min_value = value
        if value > max_value:
            max_value = value
    return (min_value, max_value)


def render_html(title: str, benchmarks: Benchmarks) -> str:
    print(f'Rendering "{title}" benchmark…')

    chartjs = {}
    for name, benchmark in benchmarks.items():
        if len(benchmark) == 0:
            chartjs[name] = None
        labels = [entry["date"] for entry in benchmark]
        base_unit = benchmark[0]["unit"]
        data = [normalize(base_unit, entry["unit"], entry["value"]) for entry in benchmark]
        min_value, max_value = min_and_max(data)
        y_scale = {"min": max(0, min_value - min_value / 3), "max": max_value + max_value / 3}
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
                    "x": {},
                    "y": {
                        "title": {"display": True, "text": base_unit},
                        **y_scale,
                    },
                }
            },
        }

    with open(os.path.join(SCRIPT_PATH, "templates/benchmark.html")) as template_file:
        html = template_file.read()
        html = html.replace("%%TITLE%%", title)
        # double encode to escape the data as a single string
        html = html.replace('"%%CHARTS%%"', json.dumps(json.dumps(chartjs)))
    return html


class Target(Enum):
    CRATES = "crates"
    SIZE = "sizes"
    ALL = "all"

    def __str__(self) -> str:
        return self.value

    def includes(self, other: Target) -> bool:
        return self is Target.ALL or self is other

    def render(self, gcs: storage.Client, end_date: datetime, num_days: int) -> dict[str, str]:
        commits_by_date = get_commits(end_date, num_days)
        out: dict[str, str] = {}
        if self.includes(Target.CRATES):
            data = get_crates_benchmark_data(gcs, commits_by_date)
            out[str(Target.CRATES)] = render_html("Rust Crates", data)
        if self.includes(Target.SIZE):
            data = get_size_benchmark_data(gcs, commits_by_date)
            out[str(Target.SIZE)] = render_html("Sizes", data)
        return out


def date_type(v: str) -> datetime:
    if v is None:
        raise Exception("asdfasdfasdfadsf")

    try:
        return datetime.strptime(v, DATE_FORMAT)
    except ValueError:
        raise argparse.ArgumentTypeError(f"Date must be in {DATE_FORMAT} format")


def days_type(v: Any) -> int:
    if v is None:
        raise Exception("asdfasdfasdfadsf")

    try:
        num_days = int(v)
        if num_days < 1:
            raise argparse.ArgumentTypeError(f"number of days must be greater than 1, got {v}")
        return num_days
    except ValueError:
        raise argparse.ArgumentTypeError(f"number of days must be a valid integer, got {v}")


class Output(Enum):
    STDOUT = "stdout"
    GCS = "gcs"
    FILE = "file"

    def parse(o: str) -> Output:
        if o == "-":
            return Output.STDOUT
        if o.startswith("gs://"):
            return Output.GCS
        return Output.FILE


@dataclass
class GcsPath:
    bucket: str
    blob: str


def parse_gcs_path(path: str) -> GcsPath:
    if not path.startswith("gs://"):
        raise ValueError(f"invalid gcs path: {path}")
    path = path.lstrip("gs://")
    try:
        bucket, blob = path.split("/", 1)
        return GcsPath(bucket, blob.rstrip("/"))
    except ValueError:
        raise ValueError(f"invalid gcs path: {path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render benchmarks from data in GCS",
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument("target", type=Target, choices=list(Target), help="Which benchmark to render")
    parser.add_argument(
        "--end-date",
        type=date_type,
        help=f"The last date to fetch, in {ESCAPED_DATE_FORMAT} format. Default: today ({datetime.today().strftime(DATE_FORMAT)})",
    )
    parser.add_argument("--num-days", type=days_type, help="How many days before end-date to fetch. Default: 30")
    parser.add_argument(
        "-o",
        "--output",
        type=str,
        required=True,
        help=textwrap.dedent(
            """\
        Directory to save to. Accepts any of:
          - '-' for stdout
          - 'gs://' prefix for GCS
          - local path
        """
        ),
    )

    args = parser.parse_args()
    target: Target = args.target
    end_date: datetime = args.end_date or datetime.today()
    num_days: int = args.num_days or 30
    output: str = args.output
    output_kind: Output = Output.parse(output)

    gcs = storage.Client()

    benchmarks = target.render(gcs, end_date, num_days)

    if output_kind is Output.STDOUT:
        for benchmark in benchmarks.values():
            print(benchmark)
    elif output_kind is Output.GCS:
        path = parse_gcs_path(output)
        print(f"Uploading to {path.bucket}/{path.blob}…")
        bucket = gcs.bucket(path.bucket)
        for name, benchmark in benchmarks.items():
            blob = bucket.blob(f"{path.blob}/{name}.html")
            blob.upload_from_string(benchmark, content_type="text/html")
    elif output_kind is Output.FILE:
        dir = Path(output)
        dir.mkdir(parents=True, exist_ok=True)
        for name, benchmark in benchmarks.items():
            (dir / f"{name}.html").write_text(benchmark)

    # runtime = get_crates_benchmark_data(gcs, get_commits(datetime.today(), 30))
    # with open(os.path.join(SCRIPT_PATH, "runtime.html"), "w") as f:
    #     f.write(render_html("runtime benchmarks", runtime))

    # sizes = get_size_benchmark_data(gcs, get_commits(datetime.today(), 30 * 6))
    # with open(os.path.join(SCRIPT_PATH, "sizes.html"), "w") as f:
    #     f.write(render_html("size benchmarks", sizes))

    # TODO: upload to gcs
    #       use on CI instead of github-action-benchmark


if __name__ == "__main__":
    main()
