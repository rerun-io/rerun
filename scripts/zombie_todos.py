#!/usr/bin/env python
from __future__ import annotations

import argparse
import os
import re
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Lock

import requests
from github import Github
from gitignore_parser import parse_gitignore
from tqdm import tqdm

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
parser.add_argument("--markdown", action="store_true", help="Format output as markdown checklist")
parser.add_argument(
    "--max-workers",
    type=int,
    default=16,
    help="Maximum number of worker threads for parallel processing (default: 16)",
)


args = parser.parse_args()


if args.GITHUB_TOKEN is None:
    print("Warning: Without GITHUB_TOKEN can only check public repos.")

if args.LINEAR_TOKEN is None:
    print("Warning: Without LINEAR_TOKEN cannot check Linear issues.")

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

internal_issue_number_pattern = re.compile(r"TODO\((?:#(\d+))(?:,\s*(?:#(\d+)))*\)")
external_issue_pattern = re.compile(r"TODO\(([a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+)#(\d+)\)")
linear_issue_pattern = re.compile(r"TODO\((RR-\d+)\)")


def collect_external_repos_from_file(path: str) -> set[str]:
    """Scan a file and collect all external repository references."""
    repos = set()
    try:
        with open(path, encoding="utf8") as f:
            content = f.read()
            matches = external_issue_pattern.findall(content)
            for repo_key, _ in matches:
                repos.add(repo_key)
    except Exception as e:
        print(f"Error reading {path}: {e}")
    return repos


def collect_linear_issues_from_file(path: str) -> set[str]:
    """Scan a file and collect all Linear issue references."""
    issues = set()
    try:
        with open(path, encoding="utf8") as f:
            content = f.read()
            matches = linear_issue_pattern.findall(content)
            for issue_id in matches:
                issues.add(issue_id)
    except Exception as e:
        print(f"Error reading {path}: {e}")
    return issues


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
    with open(path, encoding="utf8") as f:
        for i, line in enumerate(f.readlines()):
            # Check for internal issue references (TODO(#1234))
            internal_matches = internal_issue_number_pattern.search(line)
            if internal_matches is not None:
                for match in internal_matches.groups():
                    if match is not None:
                        issue_num = int(match)
                        is_closed, line_display = get_line_output(repo_owner, repo_name, issue_num, path, i, line)
                        display += line_display
                        ok &= is_closed

            # Check for external issue references (TODO(owner/repo#1234))
            external_matches = external_issue_pattern.search(line)
            if external_matches is not None:
                repo_key = external_matches.group(1)
                issue_num = int(external_matches.group(2))

                owner, name = repo_key.split("/")

                is_closed, line_display = get_line_output(owner, name, issue_num, path, i, line)
                display += line_display
                ok &= is_closed

            # Check for Linear issue references (TODO(ABC-123))
            linear_matches = linear_issue_pattern.search(line)
            if linear_matches is not None:
                issue_id = linear_matches.group(1)
                is_closed, line_display = get_linear_line_output(issue_id, path, i, line)
                display += line_display
                ok &= is_closed
    return ok, display


def process_file(filepath: str) -> tuple[bool, str]:
    """Process a single file and return True if it's OK (no zombie TODOs found)."""
    try:
        return check_file(filepath)
    except Exception as e:
        print(f"Error processing {filepath}: {e}")
        return True, ""  # Don't fail the whole process for one file


# ---


def main() -> None:
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

    exclude_paths = {
        "./CODE_STYLE.md",
        "./scripts/lint.py",
        "./scripts/zombie_todos.py",
    }

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
                if filepath.replace("\\", "/") not in exclude_paths:
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


if __name__ == "__main__":
    main()
