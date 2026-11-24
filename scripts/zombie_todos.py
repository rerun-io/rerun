#!/usr/bin/env python
"""
Hunt down zombie TODOs: TODO comments referencing closed GitHub/Linear issues.

Run with: `pixi run python scripts/zombie_todos.py`

Discovery uses GITHUB_TOKEN (env or --github-token) and LINEAR_TOKEN (env or --linear-token).
For CI, run from the checkout root with --pr <GitHub PR API response JSON>.
CI checks only closing references in the PR description and needs no API credentials or third-party packages.
Both modes skip documentation fixture files and honor NOLINT, NOLINT_START, and NOLINT_END for examples.

What this script does:
- Scans all text files in the repo for TODO comments referencing GitHub issues
  (e.g. `TODO(#1234)`, `TODO(owner/repo#1234)`) or Linear issues (e.g. `TODO(RR-1234)`).
- Checks the status of each referenced issue via the GitHub/Linear APIs.
- Reports any TODOs that reference closed issues — these are "zombie TODOs".

What to do with zombie TODOs (in rough order of likelihood):
- The TODO is stale and should be removed (the work was done, the comment is outdated).
- The TODO is a workaround for an issue that no longer exists and the workaround should be removed.
- The TODO describes remaining work that needs a new ticket — update the reference.
- The TODO is still valid and the issue needs to be re-opened.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from collections.abc import Iterator
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from threading import Lock

REPO = r"[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+"
URL_SUFFIX = r"(?:[?#][^\s<>),]*)?"
REFERENCE = re.compile(
    rf"https://github\.com/(?P<url_repo>{REPO})/issues/(?P<url_number>\d+)\b{URL_SUFFIX}"
    rf"|https://linear\.app/[^/\s]+/issue/(?P<url_linear>[a-zA-Z][a-zA-Z0-9]*-\d+)\b(?:/[a-zA-Z0-9_-]+)?{URL_SUFFIX}"
    rf"|(?P<repo>{REPO})?#(?P<number>\d+)\b"
    r"|(?P<linear>[A-Z][A-Z0-9]*-\d+)\b",
    re.IGNORECASE,
)
TODO = re.compile(r"\bTODO\(([^)\n]+)\)")


def issue_key(match: re.Match[str], repository: str) -> str:
    linear = match["url_linear"] or match["linear"]
    if linear:
        return linear.upper()
    repo = match["url_repo"] or match["repo"] or repository
    number = match["url_number"] or match["number"]
    return f"{repo.lower()}#{int(number)}"


def todo_issues(line: str, repository: str) -> set[str]:
    issues: set[str] = set()
    for todo in TODO.finditer(line):
        issues.update(issue_key(reference, repository) for reference in REFERENCE.finditer(todo[1]))
    return issues


EXCLUDED_PATHS = {"CODE_STYLE.md", "scripts/lint.py", "scripts/zombie_todos.py"}


def excluded_path(path: str) -> bool:
    """Match fixture paths relative to either the monorepo or standalone Rerun root."""
    return Path(path).as_posix().removeprefix("rerun/") in EXCLUDED_PATHS


def file_todos(path: Path, repository: str) -> Iterator[tuple[int, str, str]]:
    """Yield (one-based line number, issue, line) from text files, excluding symlinks."""
    if path.is_symlink() or not path.is_file():
        return
    data = path.read_bytes()
    if b"\0" in data:
        return
    ignored = False
    for number, line in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
        if "NOLINT_START" in line:
            ignored = True
        if "NOLINT_END" in line:
            ignored = False
        if ignored or "NOLINT" in line:
            continue
        for issue in sorted(todo_issues(line, repository)):
            yield number, issue, line


CLOSING = re.compile(
    r"\b(?:close[sd]?|closing|fix(?:es|ed|ing)?|resolve[sd]?|resolving|complete[sd]?|completing)"
    r"\b\s*:?\s*",
    re.IGNORECASE,
)
SEPARATOR = re.compile(r"(?:[ \t]*,[ \t]*(?:and\b[ \t]*)?|[ \t]+and\b[ \t]+)", re.IGNORECASE)


def read_reference(text: str, start: int, repository: str) -> tuple[str, int] | None:
    """Read a plain reference, autolink, or Markdown link at the given offset."""
    link = re.match(r"\[[^]\n]+\]\(([^)\s]+)\)", text[start:])
    if link:
        match = REFERENCE.fullmatch(link[1])
        if match:
            return issue_key(match, repository), start + link.end()
    autolink = start < len(text) and text[start] == "<"
    match = REFERENCE.match(text, start + int(autolink))
    if match:
        end = match.end()
        if autolink:
            if text[end : end + 1] != ">":
                return None
            end += 1
        return issue_key(match, repository), end
    return None


def closing_issues(text: str, repository: str) -> set[str]:
    text = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    issues: set[str] = set()
    for closing in CLOSING.finditer(text):
        reference = read_reference(text, closing.end(), repository)
        while reference:
            issue, end = reference
            issues.add(issue)
            separator = SEPARATOR.match(text, end)
            reference = read_reference(text, separator.end(), repository) if separator else None
    return issues


def find_zombies(root: Path, issues: set[str], repository: str) -> list[tuple[str, int, str]]:
    """Scan tracked text files; never follow symlinks outside the checkout."""
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).split(b"\0")
    findings = []
    for raw_path in paths:
        if not raw_path:
            continue
        path = os.fsdecode(raw_path)
        if excluded_path(path):
            continue
        # Bare references in the exported Rerun subtree retain their upstream meaning.
        todo_repository = "rerun-io/rerun" if path.startswith("rerun/") else repository
        for number, issue, _ in file_todos(root / path, todo_repository):
            if issue in issues:
                findings.append((path, number, issue))
    return findings


def check_pr(pr_path: Path) -> int:
    pr = json.loads(pr_path.read_text(encoding="utf-8"))
    repository = pr["base"]["repo"]["full_name"]
    issues = closing_issues(pr["body"] or "", repository)
    if not issues:
        print("No closing issue references found.")
        return 0
    print("Checking for zombie TODOs: " + ", ".join(sorted(issues)))
    findings = find_zombies(Path.cwd(), issues, repository)
    for path, number, issue in findings:
        message = f"This PR closes {issue}, but a TODO still references it."
        print(f"{path}:{number}: {message}")
    if findings:
        print(
            "Remove stale TODOs and obsolete workarounds, or link remaining work to an open issue. "
            "If the issue still needs work, remove its closing reference from this PR."
        )
        return 1
    print("No zombie TODOs found.")
    return 0


# ---

parser = argparse.ArgumentParser(description="Hunt down zombie TODOs.")
# To access private repositories, the token must either be a fine-grained
# generated from inside the organization or a classic token with `repo` scope.
parser.add_argument(
    "--github-token",
    dest="GITHUB_TOKEN",
    default=os.environ.get("GITHUB_TOKEN", None),
    help="Github token to fetch issues (required for API mode) (env: GITHUB_TOKEN)",
)
parser.add_argument(
    "--linear-token",
    dest="LINEAR_TOKEN",
    default=os.environ.get("LINEAR_TOKEN", None),
    help="Linear API token to fetch issues (required for Linear API mode) (env: LINEAR_TOKEN)",
)
parser.add_argument("--pr", type=Path, help="Check issues closed by this GitHub PR API response instead of live states")
parser.add_argument("--markdown", action="store_true", help="Format output as markdown checklist")
parser.add_argument(
    "--max-workers",
    type=int,
    default=16,
    help="Maximum number of worker threads for parallel processing (default: 16)",
)


args: argparse.Namespace

# --- GitHub API access ---

# Initialize GitHub client
github_client = None

# Cache for issue status checks: (repo_owner, repo_name, issue_number) -> (is_closed, author)
github_issue_cache: dict[tuple[str, str, int], tuple[bool, str]] = {}
github_cache_hit = 0  # Cache hit count
github_cache_miss = 0  # Cache miss count
github_cache_lock = Lock()  # Thread safety for the cache

# --- Linear API access ---

# Cache for Linear issue status checks: (issue_id) -> (is_closed, author)
linear_issue_cache: dict[str, tuple[bool, str]] = {}
linear_cache_hit = 0  # Cache hit count
linear_cache_miss = 0  # Cache miss count
linear_cache_lock = Lock()  # Thread safety for the cache


def init_github_client() -> None:
    from github import Github

    global github_client
    github_client = Github(args.GITHUB_TOKEN)


def check_issue_closed(repo_owner: str, repo_name: str, issue_number: int) -> tuple[bool, str]:
    """
    Check if a specific issue is closed and get its author.

    Uses caching to avoid repeated API calls for the same issue.

    Returns:
        (is_closed, author) tuple

    """
    global github_issue_cache, github_cache_hit, github_cache_miss
    cache_key = (repo_owner, repo_name, issue_number)

    # Check if we already have this result cached
    with github_cache_lock:
        if cache_key in github_issue_cache:
            github_cache_hit += 1
            return github_issue_cache[cache_key]
        github_cache_miss += 1
    # Fetch the result and cache it
    result = check_issue_closed_api(repo_owner, repo_name, issue_number)

    # Store in cache
    with github_cache_lock:
        github_issue_cache[cache_key] = result

    return result


def check_issue_closed_api(repo_owner: str, repo_name: str, issue_number: int) -> tuple[bool, str]:
    """Check if an issue is closed using PyGithub."""
    try:
        if github_client is None:
            print(f"Warning: GitHub client not initialized, skipping {repo_owner}/{repo_name}#{issue_number}")
            return False, "unknown"
        repo = github_client.get_repo(f"{repo_owner}/{repo_name}")
        issue = repo.get_issue(issue_number)
        is_closed = issue.state == "closed"
        author = issue.user.login
        return is_closed, author
    except Exception as e:
        print(f"Error fetching issue {repo_owner}/{repo_name}#{issue_number}: {e}")
        return False, "unknown"


def check_linear_issue_closed_api(issue_id: str) -> tuple[bool, str]:
    """Check if a Linear issue is closed using GraphQL API."""
    import requests

    try:
        if args.LINEAR_TOKEN is None:
            print(f"Warning: Linear token not provided, skipping Linear issue {issue_id}")
            return False, "unknown"

        # Linear GraphQL API endpoint
        url = "https://api.linear.app/graphql"

        # GraphQL query to get issue details
        query = f"""{{
        issue(id: "{issue_id}") {{
            id,
            title,
            creator {{
                name,
                email
            }},
            state {{
                name,
                type
            }}
        }}
        }}"""

        headers = {
            "Authorization": f"{args.LINEAR_TOKEN}",
            "Content-Type": "application/json",
        }

        response = requests.post(url, headers=headers, json={"query": query}, timeout=30)
        data = response.json()

        if "errors" in data:
            print(f"GraphQL errors for Linear issue {issue_id}: {data['errors']}")
            return False, "unknown"

        issue_data = data.get("data", {}).get("issue")
        if not issue_data:
            print(f"Linear issue {issue_id} not found")
            return False, "unknown"

        # Check if issue is closed (Done or Canceled states)
        state = issue_data.get("state", {})
        state_name = state.get("name", "").lower()
        state_type = state.get("type", "").lower()

        # Linear issue is considered closed if state is "Done" or "Canceled"
        is_closed = state_name in ["done", "canceled"] or state_type == "completed"

        # Get author information
        creator = issue_data.get("creator", {})
        author = creator.get("name") or creator.get("email") or "unknown"

        return is_closed, author
    except requests.exceptions.RequestException as e:
        print(f"Error fetching Linear issue {issue_id}: {e}")
        return False, "unknown"
    except Exception as e:
        print(f"Unexpected error fetching Linear issue {issue_id}: {e}")
        return False, "unknown"


def check_linear_issue_closed(issue_id: str) -> tuple[bool, str]:
    """
    Check if a Linear issue is closed (Done or Canceled) and get its author.

    Uses caching to avoid repeated API calls for the same issue.

    Returns:
        (is_closed, author) tuple

    """
    global linear_issue_cache, linear_cache_hit, linear_cache_miss
    cache_key = issue_id

    # Check if we already have this result cached
    with linear_cache_lock:
        if cache_key in linear_issue_cache:
            linear_cache_hit += 1
            return linear_issue_cache[cache_key]
        linear_cache_miss += 1

    # Fetch the result and cache it
    result = check_linear_issue_closed_api(issue_id)

    # Store in cache
    with linear_cache_lock:
        linear_issue_cache[cache_key] = result

    return result


# --- Git blame on a line ---


def get_line_blame_info(file_path: str, line_number: int, repo_path: str = ".") -> str | None:
    """
    Simpler version that uses regular git blame output.

    Args:
        file_path: Relative path to the file in the git repository
        line_number: Line number to get blame for (1-indexed)
        repo_path: Path to the git repository (default: current directory)

    Returns:
        Dictionary with blame information or None if error

    """
    try:
        # Run git blame for specific line
        cmd = [
            "git",
            "-C",
            repo_path,
            "blame",
            "-L",
            f"{line_number},{line_number}",
            "--date=iso",  # ISO format for dates
            file_path,
        ]

        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        output = result.stdout.strip()

        if not output:
            return None

        # Parse the standard git blame output
        # Format: <hash> (<author> <date> <time> <timezone> <line#>) <content>
        pattern = r"^([0-9a-f]+)\s+\(([^)]+?)\s+(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\s+[+-]\d{4})\s+\d+\)\s+(.*)$"
        match = re.match(pattern, output)

        if match:
            return f"({match.group(2).strip()} {match.group(3)})"

        return None

    except subprocess.CalledProcessError as e:
        print(f"Git blame failed: {e.stderr}")
        return None
    except Exception as e:
        print(f"Error: {e}")
        return None


# --- Check files for zombie TODOs ---

repo_owner = "rerun-io"
repo_name = "rerun"


def get_line_output(owner: str, name: str, issue_num: int, path: str, i: int, line: str) -> tuple[bool, str]:
    is_closed, author = check_issue_closed(owner, name, issue_num)
    display = ""

    if is_closed:
        blame_info = get_line_blame_info(path, i + 1)
        blame_string = f" {blame_info}" if blame_info is not None else ""
        if args.markdown:
            # Convert path to relative path for clean display
            display_path = path.lstrip("./")
            github_url = f"https://github.com/{owner}/{name}/blob/main/{display_path}#L{i + 1}"
            display = (
                f"* [ ] `{line.strip()}`\n"
                f"   * #{issue_num} (Issue author: @{author})\n"
                f"   * [`{display_path}#L{i}`]({github_url}){blame_string}\n"
            )
        else:
            display = f"{path}:{i}: {line.strip()}\n"
    return is_closed, display


def get_linear_line_output(issue_id: str, path: str, i: int, line: str) -> tuple[bool, str]:
    is_closed, author = check_linear_issue_closed(issue_id)
    display = ""

    if is_closed:
        blame_info = get_line_blame_info(path, i + 1)
        blame_string = f" {blame_info}" if blame_info is not None else ""
        if args.markdown:
            # Convert path to relative path for clean display
            display_path = path.lstrip("./")
            github_url = f"https://github.com/{repo_owner}/{repo_name}/blob/main/{display_path}#L{i + 1}"
            display = (
                f"* [ ] `{line.strip()}`\n"
                f"   * {issue_id} (Issue author: {author})\n"
                f"   * [`{display_path}#L{i}`]({github_url}){blame_string}\n"
            )
        else:
            display = f"{path}:{i}: {line.strip()}\n"
    return is_closed, display


# Returns true if the file is OK.
def check_file(path: str) -> tuple[bool, str]:
    ok = True
    display = ""
    for number, issue, line in file_todos(Path(path), f"{repo_owner}/{repo_name}"):
        if "#" in issue:
            repository, issue_num = issue.split("#")
            owner, name = repository.split("/")
            is_closed, line_display = get_line_output(owner, name, int(issue_num), path, number - 1, line)
        else:
            is_closed, line_display = get_linear_line_output(issue, path, number - 1, line)
        display += line_display
        ok &= not is_closed
    return ok, display


def process_file(filepath: str) -> tuple[bool, str]:
    """Process a single file and return True if it's OK (no zombie TODOs found)."""
    try:
        return check_file(filepath)
    except Exception as e:
        print(f"Error processing {filepath}: {e}")
        return True, ""  # Don't fail the whole process for one file


# ---


def discover() -> None:
    from gitignore_parser import parse_gitignore
    from tqdm import tqdm

    # Initialize GitHub client
    init_github_client()

    script_dirpath = os.path.dirname(os.path.realpath(__file__))
    root_dirpath = os.path.abspath(f"{script_dirpath}/..")
    os.chdir(root_dirpath)

    extensions = [
        "c",
        "cpp",
        "fbs",
        "h",
        "hpp",
        "html",
        "js",
        "md",
        "py",
        "rs",
        "sh",
        "toml",
        "txt",
        "wgsl",
        "yml",
        "cmake",
    ]

    should_ignore = parse_gitignore(".gitignore")  # TODO(emilk): parse all .gitignore files, not just top-level

    # Collect all files to process
    files_to_process = []
    print("Collecting files to process…")
    for root, dirs, files in os.walk(".", topdown=True):
        dirs[:] = [d for d in dirs if not should_ignore(d)]

        for filename in files:
            extension = filename.split(".")[-1]
            if extension in extensions:
                filepath = os.path.join(root, filename)
                if should_ignore(filepath):
                    continue
                if not excluded_path(filepath):
                    files_to_process.append(filepath)

    print(f"Processing {len(files_to_process)} files…")

    # Process files in parallel using ThreadPoolExecutor
    ok = True
    completed_files = 0
    max_workers = min(args.max_workers, len(files_to_process))  # Don't create more threads than files

    print(f"Using {max_workers} worker threads…")

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        # Submit all file processing tasks
        future_to_file = {executor.submit(process_file, filepath): filepath for filepath in files_to_process}

        # Process completed tasks with progress bar
        display = ""
        with tqdm(total=len(files_to_process), desc="Processing files", unit="file") as pbar:
            for future in as_completed(future_to_file):
                filepath = future_to_file[future]
                try:
                    file_ok, file_display = future.result()
                    ok &= file_ok
                    completed_files += 1
                    display += file_display
                    pbar.update(1)
                except Exception as e:
                    print(f"Error processing {filepath}: {e}")
                    completed_files += 1
                    pbar.update(1)
                    # Don't fail the whole process for one file error
    print(display)

    # Print cache statistics
    if github_issue_cache:
        print("\nGitHub cache statistics:")
        print(f"  Total unique issues checked: {len(github_issue_cache)}")
        closed_count = sum(1 for is_closed, _ in github_issue_cache.values() if is_closed)
        print(f"  Closed issues found: {closed_count}")
        print(f"  Open/unknown issues: {len(github_issue_cache) - closed_count}")
        print(f"  Cache hits: {github_cache_hit}, Cache misses: {github_cache_miss}")

    if linear_issue_cache:
        print("\nLinear cache statistics:")
        print(f"  Total unique issues checked: {len(linear_issue_cache)}")
        closed_count = sum(1 for is_closed, _ in linear_issue_cache.values() if is_closed)
        print(f"  Closed issues found: {closed_count}")
        print(f"  Open/unknown issues: {len(linear_issue_cache) - closed_count}")
        print(f"  Cache hits: {linear_cache_hit}, Cache misses: {linear_cache_miss}")

    if not ok:
        raise ValueError("Clean your zombies!")


def main() -> int:
    global args
    args = parser.parse_args()
    if args.pr is not None:
        return check_pr(args.pr)

    if args.GITHUB_TOKEN is None:
        print("Warning: Can only check public repos without GITHUB_TOKEN .")
    if args.LINEAR_TOKEN is None:
        print("Warning: Cannot check Linear issues without LINEAR_TOKEN .")
    discover()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
