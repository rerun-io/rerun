#!/usr/bin/env python
from __future__ import annotations

import argparse
import os
import re
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Lock

from github import Github
from gitignore_parser import parse_gitignore
from tqdm import tqdm

# ---

parser = argparse.ArgumentParser(description="Hunt down zombie TODOs.")
# To access private repositories, the token must either be a fine-grained
# generated from inside the organization or a classic token with `repo` scope.
parser.add_argument(
    "--token",
    dest="GITHUB_TOKEN",
    default=os.environ.get("GITHUB_TOKEN", None),
    help="Github token to fetch issues (required for API mode) (env: GITHUB_TOKEN)",
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

# --- GitHub API access ---

# Initialize GitHub client
github_client = None

# Cache for issue status checks: (repo_owner, repo_name, issue_number) -> (is_closed, author)
issue_cache: dict[tuple[str, str, int], tuple[bool, str]] = {}
cache_hit = 0  # Cache hit count
cache_miss = 0  # Cache miss count
cache_lock = Lock()  # Thread safety for the cache


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
    global issue_cache, cache_hit, cache_miss
    cache_key = (repo_owner, repo_name, issue_number)

    # Check if we already have this result cached
    with cache_lock:
        if cache_key in issue_cache:
            cache_hit += 1
            return issue_cache[cache_key]
        cache_miss += 1
    # Fetch the result and cache it
    result = check_issue_closed_api(repo_owner, repo_name, issue_number)

    # Store in cache
    with cache_lock:
        issue_cache[cache_key] = result

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
    if issue_cache:
        print("\nCache statistics:")
        print(f"  Total unique issues checked: {len(issue_cache)}")
        closed_count = sum(1 for is_closed, _ in issue_cache.values() if is_closed)
        print(f"  Closed issues found: {closed_count}")
        print(f"  Open/unknown issues: {len(issue_cache) - closed_count}")
        print(f"  Cache hits: {cache_hit}, Cache misses: {cache_miss}")

    if not ok:
        raise ValueError("Clean your zombies!")


if __name__ == "__main__":
    main()
