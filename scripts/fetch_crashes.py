#!/usr/bin/env python3

"""
Fetch user crashes from PostHog database, deduplicates them, and prefill issue reports for them.

Usage:
```
export POSTHOG_PERSONAL_API_KEY=<create_your_personal_api_key_in_posthog_ui>

python scripts/fetch_crashes.py -v 0.5.0 -v 0.5.1 > crashes.md
```

Optionally, you can filter out data older than a given timestamp:
```
python scripts/fetch_crashes.py -v 0.5.0 -v 0.5.1 --after 2023-05-02T20:17:52 > crashes.md
```

See Also
--------
```
python scripts/fetch_crashes.py --help
```
"""

import argparse
import json
import os
from collections import defaultdict

import requests

## CLI

parser = argparse.ArgumentParser(description="Fetch user crashes from PostHog database")
parser.add_argument(
    "-v",
    "--version",
    action="append",
    dest="versions",
    metavar="VERSION",
    help="Specify one or more Rerun version",
    required=True,
)
parser.add_argument(
    "-a",
    "--after",
    action="append",
    dest="date_after_included",
    metavar="TIMESTAMP",
    help="Filter out data older than this ISO8061 timestamp",
)
args = parser.parse_args()

## Set up query, auth, etc

personal_api_key = os.environ.get("POSTHOG_PERSONAL_API_KEY")
project_id = os.environ.get("POSTHOG_PROJECT_ID", "1954")

url = f"https://eu.posthog.com/api/projects/{project_id}/events"
properties = [
    {"key": "email", "value": "is_not_set", "operator": "is_not_set", "type": "person"},
    {"key": "rerun_version", "value": args.versions, "operator": "exact", "type": "event"},
]

## Fetch results


# NOTE: For reference, here's the complete event payload:
#
# {
#   "id": "01880cc1-10bd-0000-8338-2cc3ffed784b",
#   "distinct_id": "7981eecd-f1b9-4446-8824-b854d5474787",
#   "properties": {
#     "llvm_version": "15.0.6",
#     "target": "x86_64-pc-windows-msvc",
#     "callstack": "<omitted>",
#     "session_id": "f67f53b8-da72-4564-b849-05b048a5b6be",
#     "git_hash": "968bf7355ef146c6fad3283835f2d87e7757abc6",
#     "rerun_workspace": false,
#     "file_line": "wgpu-0.15.1/src/backend/direct.rs:3024",
#     "event_id": 1,
#     "rerun_version": "0.5.1",
#     "rust_version": "1.67.1 (d5a82bbd2 2023-02-07)",
#     "debug": false,
#     "build_date": "2023-05-02T21:24:20Z"
#   },
#   "event": "crash-panic",
#   "timestamp": "2023-05-11T20:17:52.479000+00:00",
#   "person": null,
#   "elements": [],
#   "elements_chain": ""
# }


results = []

for event in ["crash-panic", "crash-signal"]:
    params = {
        "properties": json.dumps(properties),
        "event": event,
        "orderBy": '["-timestamp"]',
    }
    if args.date_after_included:
        params["after"] = args.date_after_included
    headers = {"Authorization": f"Bearer {personal_api_key}"}

    response = requests.get(url, headers=headers, params=params)

    if response.status_code != 200:
        print("Request failed with status code:", response.status_code)
        exit(1)

    results += response.json()["results"]

## Deduplicate results and massage output

backtraces = defaultdict(list)

for res in results:
    res["properties"]["timestamp"] = res["timestamp"]
    res["properties"]["event"] = res["event"]
    res["properties"]["user_id"] = res["distinct_id"]
    backtrace = res["properties"].pop("callstack").encode("utf-8").strip()
    backtraces[backtrace].append(res.pop("properties"))


def count_uniques(backtrace):
    return len(set([prop["user_id"] for prop in backtrace[1]]))


backtraces = list(backtraces.items())
backtraces.sort(key=count_uniques, reverse=True)

## Generate reports

for backtrace, props in backtraces:
    n = count_uniques((backtrace, props))
    event = "panic" if props[0]["event"] == "crash-panic" else "signal"
    file_line = props[0].get("file_line")
    signal = props[0].get("signal")
    title = file_line if file_line is not None else signal

    timestamps = sorted(list(set([prop["timestamp"] for prop in props])))
    first_occurrence = timestamps[0]
    last_occurrence = timestamps[-1]

    targets = sorted(list(set([prop["target"] for prop in props])))
    rust_versions = sorted(list(set([prop["rust_version"] for prop in props])))
    rerun_versions = sorted(list(set([prop["rerun_version"] for prop in props])))

    print(
        f"## {n} distinct user(s) affected by {event} crash @ `{title}`\n"
        "\n"
        f"- First occurrence: `{first_occurrence}`\n"
        f"- Last occurrence: `{last_occurrence}`\n"
        f"- Affected Rust versions: `{rust_versions}`\n"
        f"- Affected Rerun versions: `{rerun_versions}`\n"
        f"- Affected Targets: `{targets}`\n"
        "\n"
        "Backtrace:\n"
        "```\n"
        f'   {backtrace.decode("utf-8")}\n'
        "```\n"
    )
