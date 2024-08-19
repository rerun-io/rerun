#!/usr/bin/env python3
"""Download traffic data for the lastest release of `rerun-io/rerun` from GitHub."""

from __future__ import annotations

import json

import requests


def main() -> None:
    response = requests.get("https://api.github.com/repos/rerun-io/rerun/releases").json()

    releases = []
    for release in response:
        if not release["draft"] and not release["prerelease"]:
            downloads = {}
            for asset in release["assets"]:
                downloads[asset["name"]] = asset["download_count"]
            releases.append({"version": release["tag_name"], "downloads": downloads})

    print(json.dumps(releases))


if __name__ == "__main__":
    main()
