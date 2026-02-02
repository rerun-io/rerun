#!/usr/bin/env python3

"""Generate a list of GitHub issues that need attention."""

from __future__ import annotations

import argparse
import multiprocessing
import sys
from typing import Any

import requests
from tqdm import tqdm

OWNER = "rerun-io"
REPO = "rerun"
OFFICIAL_RERUN_DEVS = [
    "abey79",
    "emilk",
    "jleibs",
    "jondo2010",
    "jprochazk",
    "karimo87",
    "martenbjork",
    "nikolausWest",
    "roym899",
    "teh-cmc",
    "Wumpf",
    "zehiko",
]


def get_github_token() -> str:
    import os

    token = os.environ.get("GH_ACCESS_TOKEN", "")
    if token != "":
        return token

    home_dir = os.path.expanduser("~")
    token_file = os.path.join(home_dir, ".githubtoken")

    try:
        with open(token_file, encoding="utf8") as f:
            token = f.read().strip()
        return token
    except Exception:
        pass

    print("ERROR: expected a GitHub token in the environment variable GH_ACCESS_TOKEN or in ~/.githubtoken")
    sys.exit(1)


def fetch_issue(issue_json: dict[str, Any]) -> dict[str, Any]:
    url = issue_json["url"]
    gh_access_token = get_github_token()
    headers = {"Authorization": f"Token {gh_access_token}"}
    response = requests.get(url, headers=headers)
    json: dict[str, Any] = response.json()
    if response.status_code != 200:
        print(f"ERROR {url}: {response.status_code} - {json['message']}")
        sys.exit(1)
    return json


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a list of GitHub issues that need attention.")
    parser.add_argument("--list-external", action="store_true", help="List all external issues")
    args = parser.parse_args()

    access_token = get_github_token()

    headers = {"Authorization": f"Bearer {access_token}"}

    all_issues = []

    urls = [f"https://api.github.com/repos/{OWNER}/{REPO}/issues"]

    while urls:
        url = urls.pop()

        print(f"Fetching {url}…")
        response = requests.get(url, headers=headers)
        json = response.json()
        if response.status_code != 200:
            print(f"ERROR {url}: {response.status_code} - {json['message']}")
            sys.exit(1)

        all_issues += list(json)

        # Check if there is a next page:
        if "Link" in response.headers:
            links = response.headers["Link"].split(", ")
            for link in links:
                if 'rel="next"' in link:
                    next_url = link.split(";")[0][1:-1]
                    urls += [next_url]

    pool = multiprocessing.Pool()
    issues_list = list(
        tqdm(
            pool.imap(fetch_issue, all_issues),
            total=len(all_issues),
            desc="Fetching issue details",
        ),
    )

    issues_list.sort(key=lambda issue: issue["number"])

    # Print the response content
    for issue in issues_list:
        author = issue["user"]["login"]
        html_url = issue["html_url"]
        comments = issue["comments"]
        state = issue["state"]
        labels = [label["name"] for label in issue["labels"]]

        if args.list_external and state == "open" and author not in OFFICIAL_RERUN_DEVS:
            print(f"{html_url} by {author}")
            continue

        if "👀 needs triage" in labels:
            print(f"{html_url} by {author} needs triage")
        elif len(labels) == 0:
            print(f"{html_url} by {author} has no labels")
        elif comments == 0 and state == "open" and author not in OFFICIAL_RERUN_DEVS:
            print(f"{html_url} by {author} has {comments} comments")


if __name__ == "__main__":
    main()
