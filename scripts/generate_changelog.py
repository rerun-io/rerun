#!/usr/bin/env python3

"""
Summarizes PRs since `latest` branch, grouping them based on their GitHub labels.

If the result is not satisfactory, you can edit the original PR titles and labels.
You can add the `exclude from changelog` label to minor PRs that are not of interest to our users.

Finally, copy-paste the output into `CHANGELOG.md` and add a high-level summary to the top.
"""

from __future__ import annotations

import argparse
import json
import multiprocessing
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Any

from git import Repo  # pip install GitPython
from tqdm import tqdm

OWNER = "rerun-io"
REPO = "rerun"
INCLUDE_LABELS = False  # It adds quite a bit of visual noise

# Cache for contributor classification to avoid repeated API calls
_external_contributor_cache: dict[str, bool] = {}


# Coding-agent accounts that GitHub registers as ordinary users, not as a `Bot` and with no
# `[bot]` suffix or `app/` prefix, so neither the account `type` nor the login itself flags them.
# `claude` is Anthropic's account, committing via `noreply@anthropic.com`. `cursoragent` is Cursor's.
_AGENT_USER_NAMES = {"claude", "cursoragent"}


def is_bot_account(user_name: str, account_type: str | None = None) -> bool:
    """Return whether a GitHub account is a bot rather than a human contributor.

    The account `type` from the GitHub API is the reliable signal, and the only one that catches
    bots with a plain login like the `Copilot` agent, so callers pass it whenever they have it.
    Without a `type` we judge by the login alone, which still covers GitHub App bots in both
    formats we see. The REST API uses a `[bot]` suffix, as in `dependabot[bot]` or `claude[bot]`.
    The `gh` CLI uses an `app/` prefix, as in `app/copilot-swe-agent`.
    A few coding agents run as ordinary user accounts with no such marker, so we also match
    `claude` and `cursoragent` by name.
    """
    if account_type == "Bot":
        return True
    name = user_name.lower()
    return name.endswith("[bot]") or name.startswith("app/") or name in _AGENT_USER_NAMES


def eprint(*args: Any, **kwargs: Any) -> None:
    print(*args, file=sys.stderr, **kwargs)


@dataclass
class PrInfo:
    gh_user_names: list[str]
    pr_title: str
    labels: list[str]


@dataclass
class CommitInfo:
    hexsha: str
    title: str
    pr_number: int | None
    source_ref_hash: str | None


def unique_user_names(user_names: list[str]) -> list[str]:
    """Deduplicate GitHub usernames while preserving order."""
    seen = set()
    unique = []
    for user_name in user_names:
        if user_name not in seen and not is_bot_account(user_name):
            seen.add(user_name)
            unique.append(user_name)
    return unique


def is_external_contributor(user_name: str) -> bool:
    """Return whether a GitHub user looks external to Rerun.

    We intentionally use the repository permissions endpoint instead of the org members endpoint,
    since the latter only returns public org memberships.

    Bots are never external contributors. The permissions endpoint reports the account `type`,
    so a resolvable bot like `dependabot[bot]` is caught from its type without a name list.
    Bots whose login never resolves through the API are caught earlier by `is_bot_account`,
    including the Copilot agent's `app/…` login.
    """
    if is_bot_account(user_name):
        return False

    if user_name in _external_contributor_cache:
        return _external_contributor_cache[user_name]

    try:
        result = subprocess.run(
            [
                "gh",
                "api",
                f"/repos/{OWNER}/{REPO}/collaborators/{user_name}/permission",
                "--jq",
                "[.user.type, .permission] | @tsv",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        account_type, _, permission = result.stdout.strip().partition("\t")
        is_external = not is_bot_account(user_name, account_type) and permission not in {
            "admin",
            "maintain",
            "write",
            "triage",
        }
    except subprocess.CalledProcessError as e:
        eprint(
            f"ERROR fetching repository permission for @{user_name}: {e.stderr.strip()}. Assuming external contributor."
        )
        is_external = True

    _external_contributor_cache[user_name] = is_external
    return is_external


# Slow
def fetch_pr_commit_authors(repo: str, pr_number: int) -> list[str]:
    """Return the GitHub logins of every human who authored a commit in a PR.

    GitHub resolves each commit's author email to a login server-side, so this recovers the real
    contributors even when their `Co-authored-by` trailers use a private email we cannot map.
    We pass each author's account `type` to `is_bot_account` so bot commit authors get dropped too,
    such as the `Copilot` agent whose plain login alone would not look like a bot.
    """
    try:
        result = subprocess.run(
            [
                "gh",
                "api",
                f"/repos/{OWNER}/{repo}/pulls/{pr_number}/commits?per_page=100",
                "--jq",
                ".[] | select(.author != null) | [.author.login, .author.type] | @tsv",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError as e:
        eprint(f"ERROR fetching commits for {repo} PR #{pr_number}: {e.stderr.strip()}")
        return []

    user_names = []
    for line in result.stdout.splitlines():
        login, _, account_type = line.partition("\t")
        if not is_bot_account(login, account_type):
            user_names.append(login)
    return unique_user_names(user_names)


# Slow
def fetch_reality_pr_info(commit_hash: str) -> PrInfo | None:
    """
    Fetch PR info from rerun-io/reality repo using a commit hash.
    """
    try:
        # Find PR associated with this commit
        result = subprocess.run(
            [
                "gh",
                "api",
                f"/repos/{OWNER}/reality/commits/{commit_hash}/pulls",
                "--jq",
                ".[0] | {number: .number, title: .title, labels: [.labels[].name]}",
            ],
            capture_output=True,
            text=True,
            check=True,
        )

        pr_data = json.loads(result.stdout)

        # Check if we got a PR (API returns empty object if no PR found)
        if not pr_data or "title" not in pr_data:
            return None

        pr_number = pr_data.get("number")
        # The PR's commit authors are the human contributors. We take them rather than the PR author
        # because synced PRs are opened by the sync bot, with the real people only on the commits.
        gh_user_names = fetch_pr_commit_authors("reality", pr_number) if pr_number is not None else []

        return PrInfo(
            gh_user_names=gh_user_names,
            pr_title=pr_data["title"],
            labels=pr_data["labels"],
        )

    except subprocess.CalledProcessError as e:
        # Commit doesn't exist in Reality repo, or API error
        eprint(f"Could not fetch Reality PR for commit {commit_hash[:8]}: {e.stderr.strip()}")
        return None
    except (json.JSONDecodeError, KeyError) as e:
        eprint(f"ERROR parsing Reality PR data for commit {commit_hash[:8]}: {e}")
        return None


# Slow
def fetch_pr_info_from_commit_info(commit_info: CommitInfo) -> PrInfo | None:
    """
    Fetch PR info with Reality-first, Rerun-fallback strategy.

    Priority order:
    1. Try the Reality repo using the Source-Ref commit hash, if present.
    2. Fall back to the Rerun repo using the PR number.

    Either way the contributors come from the PR's commit authors, which the API resolves to real
    logins and tags with an account `type`, so private emails are recovered and bots are dropped.
    """
    # Priority 1: Try Reality repo if Source-Ref is present
    if commit_info.source_ref_hash is not None:
        reality_pr_info = fetch_reality_pr_info(commit_info.source_ref_hash)
        if reality_pr_info is not None:
            return reality_pr_info
        # If Reality lookup fails, fall through to Rerun repo fallback

    # Priority 2: Fallback to Rerun repo using PR number
    if commit_info.pr_number is not None:
        pr_info = fetch_pr_info(commit_info.pr_number)
        if pr_info is not None:
            pr_info.gh_user_names = fetch_pr_commit_authors(REPO, commit_info.pr_number)
        return pr_info

    # No PR info available
    return None


# Slow
def fetch_pr_info(pr_number: int) -> PrInfo | None:
    try:
        # Use gh CLI to fetch PR info
        result = subprocess.run(
            [
                "gh",
                "pr",
                "view",
                str(pr_number),
                "--repo",
                f"{OWNER}/{REPO}",
                "--json",
                "title,labels,author",
            ],
            capture_output=True,
            text=True,
            check=True,
        )

        pr_data = json.loads(result.stdout)
        labels = [label["name"] for label in pr_data["labels"]]
        gh_user_name = pr_data["author"]["login"]
        return PrInfo(gh_user_names=unique_user_names([gh_user_name]), pr_title=pr_data["title"], labels=labels)

    except subprocess.CalledProcessError as e:
        eprint(
            f"ERROR fetching PR #{pr_number}: {e.stderr.strip()}. If none of these succeed, You need to install the GitHub CLI tools: https://cli.github.com/ and authenticate with github."
        )
        return None
    except (json.JSONDecodeError, KeyError) as e:
        eprint(
            f"ERROR parsing PR #{pr_number} data: {e}. If none of these succeed, You need to install the GitHub CLI tools: https://cli.github.com/ and authenticate with github."
        )
        return None


def get_commit_info(commit: Any) -> CommitInfo:
    match = re.match(r"(.*) \(#(\d+)\)$", commit.summary)
    if match:
        title = str(match.group(1))
        pr_number = int(match.group(2))
    else:
        title = commit.summary
        pr_number = None

    # Extract Source-Ref from commit body
    source_ref_hash = None
    source_ref_match = re.search(r"^Source-Ref:\s+([0-9a-f]{40})$", commit.message, re.MULTILINE)
    if source_ref_match:
        source_ref_hash = source_ref_match.group(1)

    return CommitInfo(
        hexsha=commit.hexsha,
        title=title,
        pr_number=pr_number,
        source_ref_hash=source_ref_hash,
    )


def print_section(title: str, items: list[str]) -> None:
    if 0 < len(items):
        print(f"#### {title}")
        for line in items:
            print(f"- {line}")
        print()


def commit_range(new_version: str) -> str:
    parts = new_version.split(".")
    assert len(parts) == 3, "Expected version to be on the format X.Y.Z"
    major = int(parts[0])
    minor = int(parts[1])
    patch = int(parts[2])

    if 0 < patch:
        # A patch release.
        # Include changes since last patch release.
        # This assumes we've cherry-picked stuff for this release.
        diff_since_version = f"0.{minor}.{patch - 1}"
    elif 0 < minor:
        # A minor release
        # The diff should span everything since the last minor release.
        # The script later excludes duplicated automatically, so we don't include stuff that
        # was part of intervening patch releases.
        diff_since_version = f"{major}.{minor - 1}.0"
    else:
        # A major release
        # The diff should span everything since the last major release.
        # The script later excludes duplicated automatically, so we don't include stuff that
        # was part of intervening minor/patch releases.
        diff_since_version = f"{major - 1}.{minor}.0"

    return f"{diff_since_version}..HEAD"


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a changelog.")
    parser.add_argument("--version", required=True, help="The version of the new release, e.g. 0.42.0")
    args = parser.parse_args()

    # Because how we branch, we sometimes get duplicate commits in the changelog unless we check for it
    previous_changelog = open("CHANGELOG.md", encoding="utf8").read()

    repo = Repo(".")
    commits = list(repo.iter_commits(commit_range(args.version)))
    commits.reverse()  # Most recent last
    commit_infos = list(map(get_commit_info, commits))

    pool = multiprocessing.Pool()
    pr_infos = list(
        tqdm(
            pool.imap(fetch_pr_info_from_commit_info, commit_infos),
            total=len(commit_infos),
            desc="Fetch PR info commits",
        ),
    )

    chronological = []

    # Sections:
    analytics = []
    bugs = []
    cpp = []
    dependencies = []
    dev_experience = []
    docs = []
    enhancement = []
    examples = []
    log_api = []
    oss_server = []
    mcap = []
    misc = []
    performance = []
    python = []
    refactor = []
    renderer = []
    rfc = []
    rust = []
    ui = []
    viewer = []
    web = []

    for commit_info, pr_info in zip(commit_infos, pr_infos, strict=False):
        hexsha_full = commit_info.hexsha
        hexsha_short = hexsha_full[:7]
        title = commit_info.title
        pr_number = commit_info.pr_number

        if pr_number is None and pr_info is None:
            # Someone committed straight to main without PR:
            summary = f"{title} [{hexsha_short}](https://github.com/{OWNER}/{REPO}/commit/{hexsha_full})"
            if f"[{hexsha_short}]" in previous_changelog or f"[{hexsha_full}]" in previous_changelog:
                print(f"Ignoring dup: {summary}")
                continue

            chronological.append(summary)
            misc.append(summary)
        else:
            title = pr_info.pr_title if pr_info else title  # We prefer the PR title if available
            title = title.rstrip(".").strip()  # Some PR end with an unnecessary period

            labels = pr_info.labels if pr_info else []

            if "include in changelog" not in labels and "include in OSS changelog" not in labels:
                continue

            # Generate summary - prefer PR number, fallback to commit hash
            if pr_number is not None:
                summary = f"{title} [#{pr_number}](https://github.com/{OWNER}/{REPO}/pull/{pr_number})"
                dup_checks = [f"[#{pr_number}]"]
            else:
                # No PR number in title, but we have Reality PR info - use hash of synced commit in Rerun repo
                summary = f"{title} [{hexsha_short}](https://github.com/{OWNER}/{REPO}/commit/{hexsha_full})"
                # The changelog records the short hash in brackets and the full hash in the URL, so check both.
                dup_checks = [f"[{hexsha_short}]", f"[{hexsha_full}]"]

            if any(dup_check in previous_changelog for dup_check in dup_checks):
                eprint(f"Ignoring dup: {summary}")
                continue

            chronological.append(f"{summary} {hexsha_full}")

            if INCLUDE_LABELS and 0 < len(labels):
                summary += f" ({', '.join(labels)})"

            if pr_info is not None:
                external_contributors = [
                    gh_user_name for gh_user_name in pr_info.gh_user_names if is_external_contributor(gh_user_name)
                ]
                if len(external_contributors) == 1:
                    gh_user_name = external_contributors[0]
                    summary += f" (thanks [@{gh_user_name}](https://github.com/{gh_user_name})!)"
                elif 1 < len(external_contributors):
                    thanks = ", ".join(
                        f"[@{gh_user_name}](https://github.com/{gh_user_name})"
                        for gh_user_name in external_contributors
                    )
                    summary += f" (thanks {thanks}!)"

            if labels == ["⛴ release"]:
                continue  # Ignore release PRs

            added = False

            # Some PRs can show up under multiple sections:
            if "🪵 Log & send APIs" in labels:
                log_api.append(summary)
                added = True
            else:
                if "sdk-cpp" in labels:
                    cpp.append(summary)
                    added = True
                if "sdk-python" in labels:
                    python.append(summary)
                    added = True
                if "sdk-rust" in labels:
                    rust.append(summary)
                    added = True

            if "📊 analytics" in labels:
                analytics.append(summary)
                added = True

            if not added:
                if "examples" in labels:
                    examples.append(summary)
                elif "🪳 bug" in labels or "💣 crash" in labels or "🦟 regression" in labels:
                    bugs.append(summary)
                elif "📉 performance" in labels:
                    performance.append(summary)
                elif "📖 documentation" in labels:
                    docs.append(summary)
                elif "ui" in labels:
                    ui.append(summary)
                elif "📺 re_viewer" in labels:
                    viewer.append(summary)
                elif "🔺 re_renderer" in labels:
                    renderer.append(summary)
                elif "🕸️ web" in labels:
                    web.append(summary)
                elif "🧢 MCAP" in labels:
                    mcap.append(summary)
                elif "OSS-server" in labels:
                    oss_server.append(summary)
                elif "enhancement" in labels:
                    enhancement.append(summary)
                elif "🚜 refactor" in labels:
                    refactor.append(summary)
                elif "🧑‍💻 dev experience" in labels:
                    dev_experience.append(summary)
                elif "💬 discussion" in labels:
                    rfc.append(summary)
                elif "dependencies" in labels:
                    dependencies.append(summary)
                elif not added:
                    misc.append(summary)

    print()

    # NOTE: we intentionally add TODO:s with names below, which the CI will not be happy about. Hence the # NOLINT suffixes
    print("TODO: add link to release video")  # NOLINT
    print()
    print("📖 Release blogpost: TODO: add link")  # NOLINT
    print()
    print("🧳 Migration guide: TODO: add link")  # NOLINT
    print()
    print("### ✨ Overview & highlights")
    print("TODO: fill in")  # NOLINT
    print()
    print("### ⚠️ Breaking changes")
    print("TODO: fill in")  # NOLINT
    print("🧳 Migration guide: TODO: add link (yes, again)")  # NOLINT
    print()
    print("### 🔎 Details")
    print()

    # Most interesting first:
    print_section("🪵 Log API", log_api)
    print_section("🌊 C++ API", cpp)
    print_section("🐍 Python API", python)
    print_section("🦀 Rust API", rust)
    print_section("🪳 Bug fixes", bugs)
    print_section("🌁 Viewer improvements", viewer)
    print_section("🗄️ OSS server", oss_server)
    print_section("🚀 Performance improvements", performance)
    print_section("🧑‍🏫 Examples", examples)
    print_section("📚 Docs", docs)
    print_section("🖼 UI improvements", ui)
    print_section("🕸️ Web", web)
    print_section("🎨 Renderer improvements", renderer)
    print_section("🧢 MCAP", mcap)
    print_section("✨ Other enhancement", enhancement)
    print_section("📈 Analytics", analytics)
    print_section("🗣 Merged RFCs", rfc)
    print_section("🧑‍💻 Dev-experience", dev_experience)
    print_section("🗣 Refactors", refactor)
    print_section("📦 Dependencies", dependencies)
    print_section("🤷‍ Other", misc)

    print()
    print_section("Chronological changes (don't include these)", chronological)


if __name__ == "__main__":
    main()
