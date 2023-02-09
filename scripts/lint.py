#!/usr/bin/env python3
"""
Runs custom linting on our code.

Adding "NOLINT" to any line makes the linter ignore that line.
"""

import argparse
import os
import re
import sys
from typing import Optional

todo_pattern = re.compile(r"TODO([^(]|$)")
debug_format_of_err = re.compile(r"\{\:#?\?\}.*, err")
error_match_name = re.compile(r"Err\((\w+)\)")
wasm_caps = re.compile(r"\bWASM\b")
nb_prefix = re.compile(r"\bnb_")

def lint_line(line: str) -> Optional[str]:
    if "NOLINT" in line:
        return None  # NOLINT ignores the linter

    if "FIXME" in line:
        return "we prefer TODO over FIXME"

    if "HACK" in line:
        return "we prefer TODO over HACK"

    if "todo:" in line:
        return "write 'TODO:' in upper-case"

    if "todo!()" in line:
        return 'todo!() should be written as todo!("$details")'

    if "dbg!(" in line and not line.startswith('//'):
        return 'No dbg!( in production code'

    if "unimplemented!" in line:
        return "unimplemented!(): either implement this, or rewrite it as a todo!()"

    if todo_pattern.search(line):
        return "TODO:s should be written as `TODO(yourname): what to do`"

    if "rerurn" in line.lower():
        return "Emil: you put an extra 'r' in 'Rerun' again!"

    if "{err:?}" in line or "{err:#?}" in line or debug_format_of_err.search(line):
        return "Format errors with re_error::format or using Display - NOT Debug formatting!"

    if m := re.search(error_match_name, line):
        name = m.group(1)
        # if name not in ("err", "_err", "_"):
        if name in ("e", "error"):
            return "Errors should be called 'err', '_err' or '_'"
        
    if wasm_caps.search(line):
        return "WASM should be written 'Wasm'"

    if nb_prefix.search(line):
        return "Don't use nb_things - use num_things or thing_count instead"

    return None


def test_lint() -> None:
    assert lint_line("hello world") is None

    should_pass = [
        "hello world",
        "todo lowercase is fine",
        'todo!("macro is ok with text")',
        "TODO(emilk):",
        'eprintln!("{:?}, {err}", foo)',
        'eprintln!("{:#?}, {err}", foo)',
        'eprintln!("{err}")',
        'eprintln!("{}", err)',
        'if let Err(err) = foo',
        'if let Err(_err) = foo',
        'if let Err(_) = foo',
        'WASM_FOO env var',
        'Wasm',
        'num_instances',
        'instances_count',
    ]

    should_error = [
        "FIXME",
        "HACK",
        "TODO",
        "TODO:",
        "todo!()" "unimplemented!()",
        'unimplemented!("even with text!")',
        'eprintln!("{err:?}")',
        'eprintln!("{err:#?}")',
        'eprintln!("{:?}", err)',
        'eprintln!("{:#?}", err)',
        'if let Err(error) = foo',
        'We use WASM in Rerun',
        'nb_instances',
    ]

    for line in should_pass:
        assert lint_line(line) is None, f'expected "{line}" to pass'

    for line in should_error:
        assert lint_line(line) is not None, f'expected "{line}" to fail'


def lint_file(filepath: str) -> int:
    with open(filepath) as f:
        lines_in = f.readlines()

    num_errors = 0

    for line_nr, line in enumerate(lines_in):
        error = lint_line(line)
        if error is not None:
            num_errors += 1
            print(f"{filepath}:{line_nr+1}: {error}")

    return num_errors


if __name__ == "__main__":
    test_lint() # Make sure we are bug free before we run!

    parser = argparse.ArgumentParser(description="Lint code with custom linter.")
    parser.add_argument(
        "files",
        metavar="file",
        type=str,
        nargs="*",
        help="File paths. Empty = all files, recursively.",
    )

    args = parser.parse_args()

    num_errors = 0

    if args.files:
        for filepath in args.files:
            num_errors += lint_file(filepath)
    else:
        script_dirpath = os.path.dirname(os.path.realpath(__file__))
        root_dirpath = os.path.abspath(f"{script_dirpath}/..")
        os.chdir(root_dirpath)

        extensions = ["html", "js", "py", "rs", "sh", "toml", "wgsl", "yml"]

        exclude_dirs = {"env", "venv", "target", "target_ra", "target_wasm"}

        exclude_paths = {
            "./CONTRIBUTING.md",
            "./scripts/lint.py",  # we contain all the patterns we are linting against
            "./web_viewer/re_viewer.js",  # auto-generated by wasm_bindgen
        }

        for root, dirs, files in os.walk(".", topdown=True):
            dirs[:] = [d for d in dirs if d not in exclude_dirs]
            for filename in files:
                extension = filename.split(".")[-1]
                if extension in extensions:
                    filepath = os.path.join(root, filename)
                    if filepath not in exclude_paths:
                        num_errors += lint_file(filepath)

    if num_errors == 0:
        print("lint.py finished without error")
        sys.exit(0)
    else:
        print(f"{num_errors} errors.")
        sys.exit(1)
