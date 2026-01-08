#!/usr/bin/env python3

"""
Render benchmark graphs and other tracked metrics from data in GCS.

To use this script, you must be authenticated with GCS,
see <https://cloud.google.com/docs/authentication/client-libraries> for more information.

Install dependencies:
    google-cloud-storage==3.4.1

Use the script:
    python3 scripts/ci/render_bench.py --help

    python3 scripts/ci/render_bench.py \
      all \
      --output ./benchmarks

    python3 scripts/ci/render_bench.py \
      sizes \
      --output gs://rerun-builds/graphs
"""

from __future__ import annotations

import argparse
import json
import os
import re
import textwrap
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from enum import Enum
from pathlib import Path
from subprocess import run
from typing import TYPE_CHECKING, Any

from google.cloud import storage

if TYPE_CHECKING:
    from collections.abc import Callable, Generator

SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))
DATE_FORMAT = "%Y-%m-%d"
ESCAPED_DATE_FORMAT = DATE_FORMAT.replace("%", "%%")


def non_empty_lines(s: str) -> Generator[str, None]:
    for line in s.splitlines():
        if len(line.strip()) == 0:
            continue
        yield line


@dataclass
class CommitWithDate:
    date: datetime
    commit: str


def get_commits(after: datetime) -> list[CommitWithDate]:
    # output of `git log` will be:
    # 2023-11-08 18:26:53 +0100;d694bffebae662a4dcbdd452d3a1a1b53945f871
    # 2023-11-08 18:23:02 +0100;6ce912e17c20b9d85bfe78c78a1a58bbbd2bcb29
    # 2023-11-08 18:22:11 +0100;a36bafcb5491df69ecb25af0b04833a97ba412cb

    args = ["git", "log"]
    args += [f'--after="{after.year}-{after.month}-{after.day} 00:00:00"']
    args += ["--format=%cd;%H", "--date=iso-strict"]
    log = run(args, check=True, capture_output=True, text=True).stdout.strip().splitlines()
    commits = (commit.split(";", 1) for commit in log)
    return [
        CommitWithDate(date=datetime.fromisoformat(date).astimezone(timezone.utc), commit=commit)
        for date, commit in commits
    ]


@dataclass
class Measurement:
    name: str
    value: float
    unit: str


@dataclass
class BenchmarkEntry:
    name: str
    value: float
    unit: str
    date: datetime
    commit: str
    is_duplicate: bool = False

    def duplicate(self, date: datetime) -> BenchmarkEntry:
        return BenchmarkEntry(
            name=self.name,
            value=self.value,
            unit=self.unit,
            date=date,
            commit=self.commit,
            is_duplicate=True,
        )


Benchmarks = dict[str, list[BenchmarkEntry]]


FORMAT_BENCHER_RE = re.compile(r"test\s+(\S+).*bench:\s+(\d+)\s+ns\/iter")


def parse_bencher_line(data: str) -> Measurement:
    match = FORMAT_BENCHER_RE.match(data)
    if match is None:
        raise ValueError(f"invalid bencher line: {data}")
    name, ns_iter = match.groups()
    return Measurement(name, float(ns_iter), "ns/iter")


def parse_bencher_text(data: str) -> list[Measurement]:
    return [parse_bencher_line(line) for line in non_empty_lines(data)]


def parse_sizes_json(data: str) -> list[Measurement]:
    return [
        Measurement(
            name=entry["name"],
            value=float(entry["value"]),
            unit=entry["unit"],
        )
        for entry in json.loads(data)
    ]


Blobs = dict[str, storage.Blob]


def fetch_blobs(gcs: storage.Client, bucket: str, path_prefix: str) -> Blobs:
    blobs = gcs.bucket(bucket).list_blobs(prefix=path_prefix)
    return {blob.name: blob for blob in blobs}


def collect_benchmark_data(
    commits: list[CommitWithDate],
    bucket: Blobs,
    short_sha_to_path: Callable[[str], str],
    parser: Callable[[str], list[Measurement]],
) -> Benchmarks:
    benchmarks: Benchmarks = {}

    def insert(entry: BenchmarkEntry) -> None:
        if entry.name not in benchmarks:
            benchmarks[entry.name] = []
        benchmarks[entry.name].append(entry)

    previous_entry: BenchmarkEntry | None = None
    for v in reversed(commits):
        short_sha = v.commit[0:7]

        path = short_sha_to_path(short_sha)
        if path not in bucket:
            # try to copy previous entry to maintain the graph
            if previous_entry is not None:
                insert(previous_entry.duplicate(date=v.date))
            continue  # if there is no previous entry, we just skip this one

        for measurement in parser(bucket[path].download_as_text()):
            entry = BenchmarkEntry(
                name=measurement.name,
                value=measurement.value,
                unit=measurement.unit,
                date=v.date,
                commit=v.commit,
            )
            previous_entry = entry
            insert(entry)

    return benchmarks


def get_crates_benchmark_data(gcs: storage.Client, commits: list[CommitWithDate]) -> Benchmarks:
    print('Fetching benchmark data for "Rust Crates"…')

    return collect_benchmark_data(
        commits,
        bucket=fetch_blobs(gcs, "rerun-builds", "benches"),
        short_sha_to_path=lambda short_sha: f"benches/{short_sha}",
        parser=parse_bencher_text,
    )


def get_size_benchmark_data(gcs: storage.Client, commits: list[CommitWithDate]) -> Benchmarks:
    print('Fetching benchmark data for "Sizes"…')

    return collect_benchmark_data(
        commits,
        bucket=fetch_blobs(gcs, "rerun-builds", "sizes/commit"),
        short_sha_to_path=lambda short_sha: f"sizes/commit/{short_sha}/data.json",
        parser=parse_sizes_json,
    )


BYTE_UNITS = ["b", "kb", "kib", "mb", "mib", "gb", "gib", "tb", "tib"]
VALID_CONVERSIONS = dict.fromkeys(BYTE_UNITS, BYTE_UNITS)

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
}


def convert(base_unit: str, unit: str, value: float) -> float:
    """Convert `value` from `base_unit` to `unit`."""
    if base_unit == unit:
        return value

    base_unit = base_unit.lower()
    unit = unit.lower()
    if unit not in VALID_CONVERSIONS[base_unit]:
        raise Exception(f"invalid conversion from {base_unit} to {unit}")
    return value / UNITS[unit] * UNITS[base_unit]


def min_and_max(data: list[float]) -> tuple[float, float]:
    min_value = float("inf")
    max_value = float("-inf")
    for value in data:
        min_value = min(min_value, value)
        max_value = max(max_value, value)
    return (min_value, max_value)


def render_html(title: str, benchmarks: Benchmarks) -> str:
    print(f'Rendering "{title}" benchmark…')

    def label(entry: BenchmarkEntry) -> str:
        date = entry.date.strftime("%Y-%m-%d")
        if entry.is_duplicate:
            return f"{date}"
        else:
            return f"{entry.commit[0:7]} {date}"

    chartjs: dict[str, dict[str, Any] | None] = {}
    for name, benchmark in benchmarks.items():
        if len(benchmark) == 0:
            chartjs[name] = None
            continue
        labels = [label(entry) for entry in benchmark]
        base_unit = benchmark[-1].unit
        data = [convert(base_unit, entry.unit, entry.value) for entry in benchmark]
        min_value, max_value = min_and_max(data)
        y_scale = {"min": max(0, min_value - min_value / 3), "max": max_value + max_value / 3}
        chartjs[name] = {
            "y_scale": y_scale,
            "unit": base_unit,
            "labels": labels,
            "data": data,
        }

    with open(os.path.join(SCRIPT_PATH, "templates/benchmark.html"), encoding="utf8") as template_file:
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

    def render(self, gcs: storage.Client, after: datetime) -> dict[str, str]:
        commits = get_commits(after)
        # print("commits", commits)
        out: dict[str, str] = {}

        if self.includes(Target.CRATES):
            data = get_crates_benchmark_data(gcs, commits)
            out[str(Target.CRATES)] = render_html("Rust Crates", data)

        if self.includes(Target.SIZE):
            data = get_size_benchmark_data(gcs, commits)
            out[str(Target.SIZE)] = render_html("Sizes", data)

        return out


def date_type(v: str) -> datetime:
    try:
        return datetime.strptime(v, DATE_FORMAT)
    except ValueError:
        raise argparse.ArgumentTypeError(f"Date must be in {DATE_FORMAT} format") from None


class Output(Enum):
    STDOUT = "stdout"
    GCS = "gcs"
    FILE = "file"

    @staticmethod
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
    path = path.removeprefix("gs://")
    try:
        bucket, blob = path.split("/", 1)
        return GcsPath(bucket, blob.rstrip("/"))
    except ValueError:
        raise ValueError(f"invalid gcs path: {path}") from None


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render benchmarks from data in GCS",
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument("target", type=Target, choices=list(Target), help="Which benchmark to render")
    _30_days_ago = datetime.today() - timedelta(days=30)
    parser.add_argument(
        "--after",
        type=date_type,
        help=f"The last date to fetch, in {ESCAPED_DATE_FORMAT} format. Default: today ({_30_days_ago.strftime(DATE_FORMAT)})",
    )
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
        """,
        ),
    )

    args = parser.parse_args()
    target: Target = args.target
    after: datetime = args.after or _30_days_ago
    output: str = args.output
    output_kind: Output = Output.parse(output)

    print({"target": str(target), "after": str(after), "output": output, "output_kind": str(output_kind)})

    gcs = storage.Client()

    benchmarks = target.render(gcs, after)

    # print("benchmarks", benchmarks)

    if output_kind is Output.STDOUT:
        for benchmark in benchmarks.values():
            print(benchmark)
    elif output_kind is Output.GCS:
        path = parse_gcs_path(output)
        print(f"Uploading to {path.bucket}/{path.blob}…")
        bucket = gcs.bucket(path.bucket)
        for name, benchmark in benchmarks.items():
            blob = bucket.blob(f"{path.blob}/{name}.html")
            blob.cache_control = "no-cache, max-age=0"
            blob.upload_from_string(benchmark, content_type="text/html")
    elif output_kind is Output.FILE:
        dir = Path(output)
        dir.mkdir(parents=True, exist_ok=True)
        for name, benchmark in benchmarks.items():
            (dir / f"{name}.html").write_text(benchmark)


if __name__ == "__main__":
    main()
