#!/usr/bin/env python3

"""
Remove the patch-candidate labels from merged PRs that shipped in a release.

The labels live on two repositories:
- `consider-patch` on PRs in rerun-io/rerun
- `consider-oss-patch` on PRs in rerun-io/reality

Commits synced from reality to rerun carry a `Source-Ref: <reality sha>` trailer,
which is matched against the merge commit of each labeled reality PR.
Labeled rerun PRs are matched by their merge commit, or by `(cherry picked from commit <sha>)`.
PRs whose title only matches a commit subject in the range (e.g. cherry-picks that lost both trailers) are reported for manual cleanup, not unlabelled.

Must run in a rerun-io/rerun checkout with full history and tags.
Needs `gh`. `RERUN_GH_TOKEN` and `REALITY_GH_TOKEN` select the token per repository; both fall back to `GH_TOKEN`.

Usage:
    python3 scripts/ci/remove_patch_labels.py --tag 0.39.0 --dry-run
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass

OWNER = "rerun-io"
FINAL_VERSION = re.compile(r"(\d+)\.(\d+)\.(\d+)")
SOURCE_REF = re.compile(r"^Source-Ref:\s*([0-9a-f]{40})\s*$", re.MULTILINE)
CHERRY_PICKED = re.compile(r"\(cherry picked from commit ([0-9a-f]{40})\)")


@dataclass
class Target:
    repo: str
    label: str
    token_env: str


TARGETS = [
    Target(repo="rerun", label="consider-patch", token_env="RERUN_GH_TOKEN"),
    Target(repo="reality", label="consider-oss-patch", token_env="REALITY_GH_TOKEN"),
]


@dataclass
class ReleaseCommits:
    rerun_shas: set[str]
    reality_shas: set[str]
    subjects: set[str]


def version_key(tag: str) -> tuple[int, int, int] | None:
    m = FINAL_VERSION.fullmatch(tag)
    return None if m is None else (int(m[1]), int(m[2]), int(m[3]))


def gh(args: list[str], *, token_env: str | None = None) -> str:
    env = dict(os.environ)
    if token_env is not None and os.environ.get(token_env):
        env["GH_TOKEN"] = os.environ[token_env]
    result = subprocess.run(["gh", *args], capture_output=True, text=True, env=env)
    if result.returncode != 0:
        raise RuntimeError(f"`gh {' '.join(args)}` failed: {result.stderr.strip()}")
    return result.stdout


def previous_release(tag: str) -> str:
    """The newest published final GitHub release older than `tag`; unpublished tags never reached users."""
    current = version_key(tag)
    if current is None:
        raise SystemExit(f"Not a final release tag: {tag!r}")
    releases = json.loads(
        gh(
            [
                "release",
                "list",
                f"--repo={OWNER}/rerun",
                "--exclude-drafts",
                "--exclude-pre-releases",
                "--limit=100",
                "--json=tagName",
            ],
            token_env="RERUN_GH_TOKEN",
        )
    )
    older = [(key, r["tagName"]) for r in releases if (key := version_key(r["tagName"])) and key < current]
    if not older:
        raise SystemExit(f"No published final release older than {tag}")
    return str(max(older)[1])


def strip_pr_number(title: str) -> str:
    return re.sub(r"\s*\(#\d+\)\s*$", "", title).strip()


def release_commits(previous: str, tag: str) -> ReleaseCommits:
    log = subprocess.run(
        ["git", "log", "--format=%H%x1f%B%x1e", f"{previous}..{tag}"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    commits = ReleaseCommits(rerun_shas=set(), reality_shas=set(), subjects=set())
    for entry in log.split("\x1e"):
        if not entry.strip():
            continue
        sha, _, message = entry.strip().partition("\x1f")
        commits.rerun_shas.add(sha)
        commits.rerun_shas.update(CHERRY_PICKED.findall(message))
        commits.reality_shas.update(SOURCE_REF.findall(message))
        commits.subjects.add(strip_pr_number(message.strip().splitlines()[0]))
    return commits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tag", required=True, help="The release tag, e.g. 0.39.0")
    parser.add_argument("--previous-tag", help="Start of the commit range (default: the previous published release)")
    parser.add_argument("--dry-run", action="store_true", help="List the PRs without removing any label")
    args = parser.parse_args()

    previous = args.previous_tag or previous_release(args.tag)
    commits = release_commits(previous, args.tag)
    print(
        f"{len(commits.rerun_shas)} commits in {previous}..{args.tag}, {len(commits.reality_shas)} synced from reality"
    )

    failures = 0
    for target in TARGETS:
        prs = json.loads(
            gh(
                [
                    "pr",
                    "list",
                    f"--repo={OWNER}/{target.repo}",
                    f"--label={target.label}",
                    "--state=merged",
                    "--limit=500",
                    "--json=number,title,url,mergeCommit",
                ],
                token_env=target.token_env,
            )
        )
        shas = commits.rerun_shas if target.repo == "rerun" else commits.reality_shas
        for pr in prs:
            merge_sha = (pr.get("mergeCommit") or {}).get("oid")
            if merge_sha not in shas:
                # Titles are not unique, so a title match alone is not enough to remove a label.
                if strip_pr_number(pr["title"]) in commits.subjects:
                    print(f"::notice::Check by hand, title matches but merge commit does not: {pr['url']}")
                continue
            print(f"{'Would remove' if args.dry_run else 'Removing'} '{target.label}' from {pr['url']}  {pr['title']}")
            if args.dry_run:
                continue
            try:
                gh(
                    [
                        "api",
                        "--method=DELETE",
                        f"repos/{OWNER}/{target.repo}/issues/{pr['number']}/labels/{target.label}",
                    ],
                    token_env=target.token_env,
                )
            except RuntimeError as err:
                print(f"::warning::{err}")
                failures += 1

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
