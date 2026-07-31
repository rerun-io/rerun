#!/usr/bin/env python3
"""Run clang-tidy over all C++ in `rerun_cpp`.


Requires a compilation database:

    pixi run -e cpp cpp-prepare      # writes build/debug/compile_commands.json

Usage:

    pixi run -e cpp lint-cpp-files          # analyze, non-zero exit on findings
    pixi run -e cpp lint-cpp-files --list   # just show what would be analyzed
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO_ROOT / "build" / "debug" / "compile_commands.json"
SRC_ROOT = REPO_ROOT / "rerun_cpp" / "src"

# A diagnostic can be tagged with several aliases at once, e.g.
# `[bugprone-reserved-identifier,cert-dcl37-c,cert-dcl51-cpp]`. Match the whole list so
# those are counted rather than skipped; the first name identifies the finding.
WARNING_RE = re.compile(r"\[([a-z0-9-]+(?:,[a-z0-9-]+)*)\]\s*$")


def validate_config(clang_tidy: str, probe_dir: Path) -> str | None:
    """Return an error string if `.clang-tidy` does not parse, else None.

    clang-tidy treats an unknown key as a fatal error for the whole file and then falls
    back to its built-in defaults, which report almost nothing. Without this check a
    typo'd or too-new config key produces a passing run that has analyzed nothing —
    the failure mode this project has hit before with CI that never triggered.
    """
    proc = subprocess.run(
        [clang_tidy, "--explain-config"],
        capture_output=True,
        text=True,
        cwd=probe_dir,
    )
    combined = proc.stdout + proc.stderr
    if "Error parsing" in combined or "unknown key" in combined:
        return combined.strip()
    return None


def translation_units(db_path: Path) -> list[Path]:
    """Return all .cpp files under rerun_cpp/src that the build compiles."""
    entries = json.loads(db_path.read_text())
    compiled = {Path(e["file"]).resolve() for e in entries}
    return sorted(p for p in compiled if SRC_ROOT in p.parents)


DIAG_RE = re.compile(r"^(?P<file>[^:\s][^:]*):(?P<line>\d+):(?P<col>\d+):\s+(?:warning|error|note):")


def run_one(clang_tidy: str, build_dir: Path, path: Path) -> tuple[Path, str]:
    proc = subprocess.run(
        [clang_tidy, "-p", str(build_dir), "--quiet", str(path)],
        capture_output=True,
        text=True,
    )
    return path, proc.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB, help="compile_commands.json")
    parser.add_argument("--clang-tidy", default="clang-tidy", help="clang-tidy binary")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    parser.add_argument("--list", action="store_true", help="list the files and exit")
    args = parser.parse_args()

    if not args.db.exists():
        print(
            f"error: no compilation database at {args.db}\n       run `pixi run -e cpp cpp-prepare` first",
            file=sys.stderr,
        )
        return 2

    files = translation_units(args.db)
    if not files:
        print("error: no translation units found — is the database stale?", file=sys.stderr)
        return 2

    config_error = validate_config(args.clang_tidy, SRC_ROOT)
    if config_error:
        print(f"error: .clang-tidy did not parse, so no checks would run:\n{config_error}", file=sys.stderr)
        return 2

    if args.list:
        for f in files:
            print(f.relative_to(REPO_ROOT))
        return 0

    print(f"clang-tidy: {len(files)} translation units")

    findings: list[str] = []
    compile_errors: list[Path] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = [pool.submit(run_one, args.clang_tidy, args.db.parent, f) for f in files]
        for future in concurrent.futures.as_completed(futures):
            path, out = future.result()
            if not out.strip():
                continue
            # A translation unit that does not compile is a broken build, not a lint
            # finding — report it separately so it cannot be mistaken for a clean run.
            if "clang-diagnostic-error" in out:
                compile_errors.append(path)
            findings.append(out)

    # A header is re-analyzed once per translation unit that includes it, and is reported
    # under whichever include path reached it — `archetypes/../error.hpp:103` and
    # `components/../datatypes/../error.hpp:103` are the same line of the same file. Count
    # findings by resolved location so the total reflects distinct problems, not fan-out.
    by_check: Counter[str] = Counter()
    seen: set[tuple[str, str, str, str]] = set()
    deduped_blocks: list[str] = []
    for block in findings:
        keep_block = False
        for line in block.splitlines():
            diag = DIAG_RE.match(line)
            warning = WARNING_RE.search(line)
            if not (diag and warning):
                continue
            check = warning.group(1).split(",")[0]
            key = (
                os.path.realpath(diag.group("file")),
                diag.group("line"),
                diag.group("col"),
                check,
            )
            if key in seen:
                continue
            seen.add(key)
            by_check[check] += 1
            keep_block = True
        if keep_block:
            deduped_blocks.append(block)
    findings = deduped_blocks

    if compile_errors:
        print(f"\nerror: {len(compile_errors)} translation unit(s) failed to compile:", file=sys.stderr)
        for path in sorted(compile_errors)[:10]:
            print(f"  {path.relative_to(REPO_ROOT)}", file=sys.stderr)
        print("       the build tree is incomplete — run `pixi run -e cpp cpp-build-all`", file=sys.stderr)
        return 2

    if not by_check:
        print("clean")
        return 0

    print("\n".join(findings))
    print("\nfindings by check:")
    for check, count in by_check.most_common():
        print(f"  {count:>4}  {check}")
    print(f"  {sum(by_check.values()):>4}  TOTAL")
    return 1


if __name__ == "__main__":
    sys.exit(main())
