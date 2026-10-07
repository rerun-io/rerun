#!/usr/bin/env python3

"""
Verify that a final Rerun release reached every place users get it from.

Checks:
- registries: rerun-sdk and rerun-notebook on PyPI, @rerun-io/web-viewer on npm, rerun on crates.io.
  conda-forge lags behind by hours to days, so a missing conda-forge version is only a warning.
- github-release: published, not a prerelease, marked latest, with assets.
- website: rerun.io/docs and rerun.io/viewer serve the release, and app.rerun.io, the changeset page,
  and the Python reference docs exist for it.
- docs-latest: the `docs-latest` branch contains the release tag.
- gradio: rerun-io/gradio-rerun-viewer has a published release with the same version tag.

With `--wait-minutes`, first polls the website and gradio checks until they pass or time runs out,
since both only finish some time after the release is published.

Prints a summary, also appended to `$GITHUB_STEP_SUMMARY` when set, and exits non-zero if any check failed.
Set `GH_TOKEN` (or `GITHUB_TOKEN`) to avoid GitHub API rate limits.

Usage:
    python3 scripts/ci/verify_release.py 0.39.0
    python3 scripts/ci/verify_release.py 0.39.0 --wait-minutes 90
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Literal

REPO = "rerun-io/rerun"
GRADIO_REPO = "rerun-io/gradio-rerun-viewer"
GRADIO_WORKFLOW_URL = f"https://github.com/{GRADIO_REPO}/actions/workflows/auto_release_on_rerun.yml"
USER_AGENT = "rerun-release-verification"
# The website redeploy and the gradio release each take tens of minutes; polling faster only adds log noise.
POLL_INTERVAL_SECS = 120


class CheckFailed(Exception):
    pass


class CheckWarning(Exception):
    pass


@dataclass
class Result:
    name: str
    status: Literal["ok", "warning", "failed"]
    message: str


def http_get(url: str, *, headers: dict[str, str] | None = None) -> tuple[int, str]:
    """GET `url` following redirects; returns (status, body). Status 0 means no response."""
    if not url.startswith("https://"):
        raise ValueError(f"Not an https URL: {url}")
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, **(headers or {})})  # noqa: S310
    try:
        with urllib.request.urlopen(request, timeout=60) as response:  # noqa: S310
            return response.status, response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as err:
        return err.code, ""
    except (urllib.error.URLError, TimeoutError) as err:
        print(f"  {url}: {err}")
        return 0, ""


def gh_headers() -> dict[str, str]:
    headers = {"Accept": "application/vnd.github+json"}
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def gh_api(path: str) -> Any:
    status, body = http_get(f"https://api.github.com/{path}", headers=gh_headers())
    if status != 200:
        raise CheckFailed(f"GitHub API answered HTTP {status} for {path}")
    return json.loads(body)


def parse_final_version(version: str) -> tuple[int, int, int]:
    m = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", version)
    if m is None:
        raise SystemExit(f"Not a final release version: {version!r} (expected e.g. 0.39.0)")
    return int(m[1]), int(m[2]), int(m[3])


# ---------------------------------------------------------------------------------------------------------------------
# Checks


def check_registries(version: str) -> str:
    urls = {
        "PyPI rerun-sdk": f"https://pypi.org/pypi/rerun-sdk/{version}/json",
        "PyPI rerun-notebook": f"https://pypi.org/pypi/rerun-notebook/{version}/json",
        "npm @rerun-io/web-viewer": f"https://registry.npmjs.org/@rerun-io/web-viewer/{version}",
        "crates.io rerun": f"https://crates.io/api/v1/crates/rerun/{version}",
    }
    missing = []
    for what, url in urls.items():
        status, _ = http_get(url)
        print(f"  {what}: HTTP {status}")
        if status != 200:
            missing.append(what)
    if missing:
        raise CheckFailed(f"{version} missing from " + ", ".join(missing))
    return "PyPI, npm and crates.io"


def check_conda(version: str) -> str:
    status, body = http_get("https://api.anaconda.org/package/conda-forge/rerun-sdk")
    versions = json.loads(body).get("versions", []) if status == 200 else []
    print(f"  conda-forge rerun-sdk: HTTP {status}, latest {versions[-1] if versions else '?'}")
    if version not in versions:
        raise CheckWarning(
            f"{version} not on conda-forge yet: check https://github.com/conda-forge/rerun-sdk-feedstock/pulls"
        )
    return "conda-forge"


def check_github_release(version: str) -> str:
    release = gh_api(f"repos/{REPO}/releases/tags/{version}")
    print(f"  {release['html_url']}: prerelease={release['prerelease']} assets={len(release.get('assets', []))}")
    problems = []
    if release["prerelease"]:
        problems.append("marked as prerelease")
    if not release.get("assets"):
        problems.append("no assets attached: check the `Sync Release Assets` job")
    latest = gh_api(f"repos/{REPO}/releases/latest")["tag_name"]
    if latest != version:
        problems.append(f"'latest' release is {latest}")
    if problems:
        raise CheckFailed("; ".join(problems))
    return "published, latest, with assets"


def check_website(version: str) -> str:
    major, minor, _ = parse_final_version(version)
    problems = []

    status, docs = http_get("https://rerun.io/docs")
    m = re.search(r'latestVersion:"([^"]+)"', docs)
    print(f"  rerun.io/docs latest version: {m.group(1) if m else '?'}")
    if status != 200 or m is None or m.group(1) != version:
        problems.append(f"rerun.io/docs reports {m.group(1) if m else 'no version'}")

    status, viewer = http_get("https://rerun.io/viewer")
    m = re.search(r'type:"version",slug:"([^"]+)"', viewer)
    print(f"  rerun.io/viewer version: {m.group(1) if m else '?'}")
    if status != 200 or m is None or m.group(1) != version:
        problems.append(f"rerun.io/viewer serves {m.group(1) if m else 'no version'}")

    for url in [
        f"https://app.rerun.io/version/{version}/re_viewer_bg.wasm",
        f"https://rerun.io/docs/changelog/changeset-{major}-{minor}",
        f"https://ref.rerun.io/docs/python/{version}/",
    ]:
        status, _ = http_get(url)
        print(f"  {url}: HTTP {status}")
        if status != 200:
            problems.append(f"HTTP {status} for {url}")
    if problems:
        raise CheckFailed("; ".join(problems))
    return "rerun.io/docs, rerun.io/viewer, app.rerun.io"


def check_docs_latest(version: str) -> str:
    compare = gh_api(f"repos/{REPO}/compare/{version}...docs-latest")
    print(f"  docs-latest is {compare['status']} of {version}")
    if compare["behind_by"] != 0:
        raise CheckFailed(f"docs-latest does not contain {version}")
    return f"contains {version}"


def check_gradio(version: str) -> str:
    # `auto_release_on_rerun.yml` ends by publishing a gradio-rerun-viewer release with the same tag.
    status, body = http_get(f"https://api.github.com/repos/{GRADIO_REPO}/releases/tags/{version}", headers=gh_headers())
    if status == 404:
        raise CheckFailed(f"{GRADIO_REPO} has no {version} release yet: {GRADIO_WORKFLOW_URL}")
    if status != 200:
        raise CheckFailed(f"HTTP {status} looking up the {GRADIO_REPO} {version} release")
    release = json.loads(body)
    print(f"  {release['html_url']}: draft={release['draft']}")
    if release["draft"]:
        raise CheckFailed(f"{GRADIO_REPO} {version} release is still a draft: {release['html_url']}")
    return f"{GRADIO_REPO} {version} released"


CHECKS: dict[str, Callable[[str], str]] = {
    "registries": check_registries,
    "conda-forge": check_conda,
    "github-release": check_github_release,
    "website": check_website,
    "docs-latest": check_docs_latest,
    "gradio": check_gradio,
}


# ---------------------------------------------------------------------------------------------------------------------


def run_check(name: str, version: str) -> Result:
    print(f"\n== {name}", flush=True)
    try:
        return Result(name, "ok", CHECKS[name](version))
    except CheckWarning as err:
        print(f"  WARNING: {err}")
        return Result(name, "warning", str(err))
    except (CheckFailed, KeyError, ValueError) as err:
        print(f"  FAILED: {err}")
        return Result(name, "failed", str(err))


def wait_for_redeploys(version: str, minutes: float) -> None:
    """Polls the checks that lag behind publishing (the website redeploy and the gradio release) until they pass."""
    deadline = time.monotonic() + minutes * 60
    while any(run_check(name, version).status != "ok" for name in ("website", "gradio")):
        if time.monotonic() + POLL_INTERVAL_SECS > deadline:
            print(f"Still not done after {minutes:g} minutes; checking everything anyway")
            return
        print(f"Waiting {POLL_INTERVAL_SECS} s for the website and gradio release …", flush=True)
        time.sleep(POLL_INTERVAL_SECS)


def markdown_summary(version: str, results: list[Result]) -> str:
    icons = {"ok": "✅", "warning": "⚠️", "failed": "❌"}
    failed = sum(r.status == "failed" for r in results)
    headline = (
        f"Release verification for {version}: {failed} failed"
        if failed
        else f"Release verification for {version} passed"
    )
    lines = [f"### {headline}", ""] + [f"- {icons[r.status]} **{r.name}**: {r.message}" for r in results]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("version", help="Released version, e.g. 0.39.0")
    parser.add_argument(
        "--wait-minutes", type=float, default=0, help="Poll the website and gradio release for up to this long first"
    )
    parser.add_argument("--only", action="append", choices=sorted(CHECKS), default=[], help="Run only this check")
    args = parser.parse_args()
    parse_final_version(args.version)

    if args.wait_minutes > 0:
        wait_for_redeploys(args.version, args.wait_minutes)

    results = [run_check(name, args.version) for name in CHECKS if not args.only or name in args.only]

    summary = markdown_summary(args.version, results)
    print(f"\n{summary}")

    if step_summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(step_summary, "a", encoding="utf-8") as f:
            f.write(summary + "\n")

    return 1 if any(r.status == "failed" for r in results) else 0


if __name__ == "__main__":
    sys.exit(main())
