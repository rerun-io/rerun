#!/usr/bin/env python3
"""
Check links only in lines added by a PR.

This script extracts lines added in a PR and runs lychee on them to avoid
checking links in the entire codebase on every PR.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import tomlkit


def eprint(*args: Any, **kwargs: Any) -> None:
    """Prints a message to stderr."""
    print(*args, file=sys.stderr, **kwargs)


def load_lychee_excludes(config_path: str = "lychee.toml") -> list[str]:
    """Load and normalize exclude_path patterns from lychee config file."""
    try:
        with open(config_path, encoding="utf-8") as f:
            config = tomlkit.load(f)
            exclude_paths = config.get("exclude_path", [])

            # Validate that exclude_path is actually a list
            if not isinstance(exclude_paths, list):
                eprint(f"Error: 'exclude_path' in {config_path} must be a list, got {type(exclude_paths).__name__}")
                sys.exit(1)

            # Normalize patterns (strip leading ./) and validate each is a string
            normalized = []
            for pattern in exclude_paths:
                if not isinstance(pattern, str):
                    eprint(f"Error: exclude_path patterns must be strings, got {type(pattern).__name__}: {pattern}")
                    sys.exit(1)
                normalized.append(pattern.lstrip("./"))

            return normalized
    except FileNotFoundError:
        eprint(f"Error: lychee config file '{config_path}' not found.")
        eprint("This file is required to determine which files to exclude from link checking.")
        sys.exit(1)
    except Exception as e:
        eprint(f"Error: Failed to parse lychee config '{config_path}': {e}")
        eprint("Please ensure the config file is valid TOML format.")
        sys.exit(1)


def should_exclude_file(filepath: str, exclude_patterns: list[str]) -> bool:
    """Check if a file matches any exclude pattern from lychee config."""
    if not exclude_patterns:
        return False

    normalized = filepath.lstrip("./")
    for pattern in exclude_patterns:
        # Exact match or directory prefix match
        if normalized == pattern or normalized.startswith(pattern + "/"):
            return True
        # Glob pattern match
        if fnmatch.fnmatch(normalized, pattern):
            return True
    return False


def get_added_lines_with_links(
    base_ref: str = "origin/main", exclude_patterns: list[str] | None = None
) -> dict[str, list[str]]:
    """
    Get lines added in the current branch that contain URLs.

    Returns a dict mapping filenames to lists of lines containing links.
    """
    if exclude_patterns is None:
        exclude_patterns = []

    # Get the diff of added lines (try committed changes first, then staged changes)
    # Disable external diff tools to get standard git diff format
    env = os.environ.copy()
    env["GIT_EXTERNAL_DIFF"] = ""

    try:
        result = subprocess.run(
            ["git", "diff", "--no-ext-diff", "--no-merges", "--diff-filter=AM", f"{base_ref}...HEAD"],  # NOLINT
            capture_output=True,
            text=True,
            check=True,
            env=env,
        )
        # If no committed changes, try staged changes
        if not result.stdout.strip():
            result = subprocess.run(
                ["git", "diff", "--no-ext-diff", "--cached", "--no-merges", "--diff-filter=AM"],
                capture_output=True,
                text=True,
                check=True,
                env=env,
            )
    except subprocess.CalledProcessError as e:
        eprint(f"Error getting git diff: {e}")
        return {}

    lines_by_file: dict[str, list[str]] = {}
    current_file: str | None = None

    for line in result.stdout.split("\n"):
        if line.startswith("+++"):
            # Extract filename from +++ b/path/to/file
            if line.startswith("+++ b/"):
                current_file = line[6:]  # Remove '+++ b/'
                # Skip files that match lychee exclude patterns
                if should_exclude_file(current_file, exclude_patterns):
                    current_file = None
                elif current_file not in lines_by_file:
                    lines_by_file[current_file] = []
            else:
                current_file = None
        elif line.startswith("+") and not line.startswith("+++") and current_file:
            # This is an added line, check if it contains URLs
            line_content = line[1:]  # Remove the '+' prefix
            if (
                "http://" in line_content
                or "https://" in line_content
                or "ftp://" in line_content
                or "file://" in line_content
                or re.search(r"\[.+\]\(.+\)", line_content)  # Markdown links [text](url)
            ):
                lines_by_file[current_file].append(line_content)

    # Remove empty entries
    return {filename: lines for filename, lines in lines_by_file.items() if lines}


def extract_links_from_line(line: str) -> list[str]:
    """Extract all links from a line of text."""
    links = []

    # Find markdown-style links [text](url)
    markdown_links = re.findall(r'\[.+?\]\((.+?)\)', line)
    links.extend(markdown_links)

    # Find bare URLs
    url_pattern = r'https?://[^\s<>"\[\]{}|\\^`]+|ftp://[^\s<>"\[\]{}|\\^`]+|file://[^\s<>"\[\]{}|\\^`]+'
    bare_urls = re.findall(url_pattern, line)
    links.extend(bare_urls)

    return links


def resolve_relative_link(link: str, source_file_path: str) -> str:
    """Resolve a relative link to an absolute path or return unchanged if already absolute."""
    # Parse the URL to check if it's already absolute
    parsed = urlparse(link)
    if parsed.scheme:  # Already has a scheme (http, https, ftp, file, etc.)
        return link

    # Handle relative paths
    source_dir = Path(source_file_path).parent

    # Remove any URL fragments or query parameters for path resolution
    link_path = link.split('#')[0].split('?')[0]

    try:
        # Resolve the relative path
        resolved_path = (source_dir / link_path).resolve()

        # Check if the resolved file exists
        if resolved_path.exists():
            # Convert to file:// URL, preserving any fragments
            file_url = resolved_path.as_uri()
            if '#' in link:
                file_url += '#' + link.split('#', 1)[1]
            return file_url
        else:
            # File doesn't exist, but keep the resolved absolute path for lychee to report
            abs_path = str(resolved_path)
            if '#' in link:
                abs_path += '#' + link.split('#', 1)[1]
            return abs_path
    except (OSError, ValueError):
        # If path resolution fails, return the original link
        return link


class TempLinkFile:
    def __init__(self, path: str, source_file: str) -> None:
        self.path = path
        self.source_file = source_file


def create_temp_files(lines_by_file: dict[str, list[str]]) -> list[TempLinkFile]:
    """
    Create temporary files with resolved links from the lines that contain links.

    Returns a list of temporary file paths.
    """
    temp_files = []

    for file, lines in lines_by_file.items():
        if not lines:
            continue

        # Create temp file with appropriate extension
        ext = Path(file).suffix
        fd, temp_path = tempfile.mkstemp(suffix=f"{ext}", prefix="pr_links_")

        try:
            with os.fdopen(fd, "w") as f:
                for line in lines:
                    # Extract and resolve all links in this line
                    links = extract_links_from_line(line)
                    resolved_line = line

                    for link in links:
                        resolved_link = resolve_relative_link(link, file)
                        # Replace the original link with the resolved one in the line
                        resolved_line = resolved_line.replace(link, resolved_link)

                    f.write(resolved_line + "\n")

            # TODO(lycheeverse/lychee#972): Windows absolute paths don't work.
            # But looks like UNC paths work!
            if sys.platform == "win32":
                temp_path = "\\\\.\\" + temp_path

            temp_files.append(TempLinkFile(temp_path, file))
        except Exception:
            os.unlink(temp_path)
            raise

    return temp_files


def run_lychee(temp_files: list[TempLinkFile]) -> int:
    """
    Run lychee on the temporary files.

    Returns the exit code from lychee.
    """
    if not temp_files:
        eprint("No files with links found in added lines.")
        return 0

    failed = False

    # Now that relative links have been resolved to absolute paths,
    # we can run lychee on each temp file without needing base-url
    for temp_file in temp_files:
        # Build lychee command
        cmd = [
            "lychee",
            "--verbose",
            "--cache",
            "--max-cache-age",
            "1d",
            temp_file.path,
        ]

        eprint(f"Running lychee on new links in {temp_file.source_file}: {' '.join(cmd)}")

        try:
            result = subprocess.run(cmd, check=False)
            if result.returncode != 0:
                failed = True
            eprint()
        except FileNotFoundError:
            eprint("Error: lychee not found. Please install lychee.")
            return 1

    return 1 if failed else 0


def cleanup_temp_files(temp_files: list[TempLinkFile]) -> None:
    """Clean up temporary files."""
    for temp_file in temp_files:
        try:
            os.unlink(temp_file.path)
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Check links in PR-added lines only")
    parser.add_argument(
        "--base-ref", default="origin/main", help="Base reference to compare against (default: origin/main)"
    )
    parser.add_argument("--dry-run", action="store_true", help="Show what would be checked without running lychee")
    parser.add_argument("--no-cleanup", action="store_true", help="Don't clean up temporary files")

    args = parser.parse_args()

    # Load lychee exclude patterns
    exclude_patterns = load_lychee_excludes()

    # Get lines with links from the diff
    lines_by_file = get_added_lines_with_links(args.base_ref, exclude_patterns)

    if not lines_by_file:
        eprint("No added lines with links found.")
        return 0

    if args.dry_run:
        eprint("Would check the following lines:")
        for file, lines in lines_by_file.items():
            eprint(f"\n{file}:")
            for line in lines:
                eprint(f"  {line}")
        return 0

    # Create temporary files
    temp_files = create_temp_files(lines_by_file)

    try:
        # Run lychee
        exit_code = run_lychee(temp_files)
        return exit_code
    finally:
        # Clean up
        if not args.no_cleanup:
            cleanup_temp_files(temp_files)


if __name__ == "__main__":
    sys.exit(main())
