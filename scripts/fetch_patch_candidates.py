#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Any

from colorama import Fore, Style
import pandas as pd
from tabulate import tabulate
from tqdm import tqdm

DOC = """
Fetch potential patch release candidates from both rerun-io/rerun and rerun-io/reality.

Looks at merged PRs labeled:
- "consider-patch" in rerun-io/rerun
- "consider-oss-patch" in rerun-io/reality

Requirements:
- gh CLI (https://cli.github.com/) authenticated with GitHub
- 3rd-party packages are part of the UV workspace
"""

MAX_COLUMN_WIDTH = 50

OWNER = "rerun-io"
RERUN_LABEL = "consider-patch"
REALITY_LABEL = "consider-oss-patch"


@dataclass
class Release:
    tag: str
    published_at: str


@dataclass
class PullRequest:
    repo: str
    number: int
    title: str
    url: str
    merged_at: str
    merge_commit_sha: str | None
    author: str


@dataclass
class PatchCandidate:
    prs: list[PullRequest]
    merged: bool
    rerun_sha: str | None
    reality_sha: str | None
    warning: str | None = None


def eprint(*args: object, **kwargs: Any) -> None:
    print(*args, file=sys.stderr, **kwargs)


def strip_pr_number(title: str) -> str:
    """Strip trailing PR number from title: 'some commit (#123)' -> 'some commit'."""
    return re.sub(r"\s*\(#\d+\)\s*$", "", title).strip()


def fetch_prs(repo: str, label: str, state: str) -> list[PullRequest]:
    """Fetch PRs with the given label and state from a GitHub repo."""
    try:
        result = subprocess.run(
            [
                "gh",
                "pr",
                "list",
                "--repo",
                f"{OWNER}/{repo}",
                "--label",
                label,
                "--state",
                state,
                "--json",
                "number,title,url,mergeCommit,mergedAt,author",
                "--limit",
                "100",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        raw: list[dict[str, Any]] = json.loads(result.stdout)
        prs = []
        for pr in raw:
            author_info = pr.get("author")
            if author_info and author_info.get("name"):
                author = author_info["name"]
            elif author_info and author_info.get("login"):
                author = author_info["login"]
            else:
                author = "failed to get author"

            prs.append(
                PullRequest(
                    repo=repo,
                    number=pr["number"],
                    title=pr["title"],
                    url=pr["url"],
                    merged_at=pr.get("mergedAt", ""),
                    merge_commit_sha=(pr.get("mergeCommit") or {}).get("oid"),
                    author=author,
                )
            )
        prs.sort(key=lambda pr: pr.merged_at)
        return prs
    except subprocess.CalledProcessError as e:
        eprint(f"ERROR fetching PRs from {OWNER}/{repo}: {e.stderr.strip()}")
        eprint("Make sure gh CLI is installed and authenticated: https://cli.github.com/")
        sys.exit(1)


def fetch_rerun_releases() -> list[Release]:
    """Fetch recent releases from rerun-io/rerun, sorted by date ascending."""
    try:
        result = subprocess.run(
            [
                "gh",
                "release",
                "list",
                "--repo",
                f"{OWNER}/rerun",
                "--json",
                "tagName,publishedAt",
                "--limit",
                "50",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        raw = json.loads(result.stdout)
        releases = [Release(tag=r["tagName"], published_at=r.get("publishedAt", "")) for r in raw]
        releases.sort(key=lambda r: r.published_at)
        return releases
    except subprocess.CalledProcessError as e:
        eprint(f"Warning: could not fetch releases: {e.stderr.strip()}")
        return []


def find_release_after(merged_at: str, releases: list[Release]) -> Release | None:
    """Find the earliest rerun release published after the given merge date."""
    for release in releases:
        if "prerelease" in release.tag:
            continue
        if release.published_at > merged_at:
            return release
    return None


def maybe_warn(merged_at: str, releases: list[Release]) -> str | None:
    """Creates a warning if a release was published after merge, which may indicate an outdated label."""
    release = find_release_after(merged_at, releases) if merged_at else None
    return f"{release.tag} released after merge! Outdated label?" if release else None


def find_commit_via_github(repo: str, message: str) -> str | None:
    """Search for a commit by message using GitHub's commit search API."""
    escaped = message.replace('"', '\\"')
    query = f'"{escaped}" repo:{OWNER}/{repo}'
    try:
        result = subprocess.run(
            [
                "gh",
                "api",
                "--method",
                "GET",
                "search/commits",
                "-f",
                f"q={query}",
                "--jq",
                ".items[0].sha",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        sha = result.stdout.strip()
        return sha if sha and sha != "null" else None
    except subprocess.CalledProcessError:
        return None


def main() -> None:
    parser = argparse.ArgumentParser(
        description=DOC,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.parse_args()

    with tqdm(total=5, desc="Fetching data", file=sys.stderr, leave=False) as pbar:
        pbar.set_description("Fetching rerun releases")
        releases = fetch_rerun_releases()
        pbar.update(1)

        pbar.set_description(f"Fetching merged '{RERUN_LABEL}' PRs from rerun")
        rerun_prs = fetch_prs("rerun", RERUN_LABEL, "merged")
        pbar.update(1)

        pbar.set_description(f"Fetching merged '{REALITY_LABEL}' PRs from reality")
        reality_prs = fetch_prs("reality", REALITY_LABEL, "merged")
        pbar.update(1)

        pbar.set_description(f"Fetching open '{RERUN_LABEL}' PRs from rerun")
        rerun_open = fetch_prs("rerun", RERUN_LABEL, "open")
        pbar.update(1)

        pbar.set_description(f"Fetching open '{REALITY_LABEL}' PRs from reality")
        reality_open = fetch_prs("reality", REALITY_LABEL, "open")
        pbar.update(1)

    all_results: list[PatchCandidate] = []

    # Open (unmerged) PRs — no commit resolution needed.
    for pr in rerun_open + reality_open:
        all_results.append(PatchCandidate(prs=[pr], merged=False, rerun_sha=None, reality_sha=None))

    # Merged PRs — resolve commits across repos.
    merged_prs = rerun_prs + reality_prs
    progress = tqdm(merged_prs, desc="Resolving commits", file=sys.stderr, leave=False)
    for pr in progress:
        progress.set_description(pr.url)
        search_msg = strip_pr_number(pr.title)

        if pr.repo == "rerun":
            rerun_sha = pr.merge_commit_sha
            reality_sha = find_commit_via_github("reality", search_msg)
        else:
            rerun_sha = find_commit_via_github("rerun", search_msg)
            reality_sha = pr.merge_commit_sha

        all_results.append(
            PatchCandidate(
                prs=[pr],
                merged=True,
                rerun_sha=rerun_sha,
                reality_sha=reality_sha,
                warning=maybe_warn(pr.merged_at, releases),
            )
        )

    # Deduplicate entries that appear in both repos (repo sync creates matching commits).
    deduped: dict[str, PatchCandidate] = {}
    for c in all_results:
        key = strip_pr_number(c.prs[0].title)
        if key in deduped:
            existing = deduped[key]
            existing.rerun_sha = existing.rerun_sha or c.rerun_sha
            existing.reality_sha = existing.reality_sha or c.reality_sha
            existing.warning = existing.warning or c.warning
            existing.prs.extend(c.prs)
        else:
            deduped[key] = c
    all_results = list(deduped.values())

    # Ensure that the PRs from both repos are sorted by time.
    # Note: this here works because we have ISO timestamps, which sort lexicographically.
    all_results.sort(key=lambda c: c.prs[0].merged_at or "")

    # Warn about unmerged PRs that still carry the patch label.
    for c in all_results:
        if not c.merged:
            eprint(
                f"{Fore.YELLOW}WARNING: {c.prs[0].url} has patch label but isn't merged yet! ({c.prs[0].author}){Style.RESET_ALL}"
            )

    merged_results = [c for c in all_results if c.merged]
    if not merged_results:
        eprint("No merged patch candidates found in either repository.")
        return

    def short_sha(sha: str | None) -> str:
        return sha[:8] if sha else "—"

    def merge_date(candidate: PatchCandidate) -> str:
        return candidate.prs[0].merged_at[:10] if candidate.prs[0].merged_at else "—"

    columns: dict[str, list[str]] = {
        "Merge date": [merge_date(c) for c in merged_results],
        "rerun": [short_sha(c.rerun_sha) for c in merged_results],
        "reality": [short_sha(c.reality_sha) for c in merged_results],
        "Origin PR": ["\n".join(p.url for p in c.prs) for c in merged_results],
        "Commit message": [c.prs[0].title for c in merged_results],
        "Author": [c.prs[0].author for c in merged_results],
    }
    has_warnings = any(c.warning for c in merged_results)
    if has_warnings:
        columns[f"{Fore.YELLOW}WARNING{Style.RESET_ALL}"] = [c.warning or "" for c in merged_results]

    df = pd.DataFrame(columns)
    print(
        tabulate(
            df.values.tolist(),
            headers=list(df.columns),
            tablefmt="rounded_grid",
            stralign="left",
            maxcolwidths=MAX_COLUMN_WIDTH,
        )
    )


if __name__ == "__main__":
    main()
