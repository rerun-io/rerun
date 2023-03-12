#!/usr/bin/env python3

"""
Runs custom linting on our code.

Adding "NOLINT" to any line makes the linter ignore that line.
"""

import argparse
import os
import re
import sys
from typing import Any, List, Optional, Tuple

# -----------------------------------------------------------------------------

todo_pattern = re.compile(r"TODO([^(]|$)")
debug_format_of_err = re.compile(r"\{\:#?\?\}.*, err")
error_match_name = re.compile(r"Err\((\w+)\)")
wasm_caps = re.compile(r"\bWASM\b")
nb_prefix = re.compile(r"\bnb_")


def lint_line(line: str) -> Optional[str]:
    if "NOLINT" in line:
        return None  # NOLINT ignores the linter

    if "Github" in line:
        return "It's 'GitHub', not 'Github'"

    if " github " in line:
        return "It's 'GitHub', not 'github'"

    if "FIXME" in line:
        return "we prefer TODO over FIXME"

    if "HACK" in line:
        return "we prefer TODO over HACK"

    if "todo:" in line:
        return "write 'TODO:' in upper-case"

    if "todo!()" in line:
        return 'todo!() should be written as todo!("$details")'

    if "unimplemented!" in line:
        return "unimplemented!(): either implement this, or rewrite it as a todo!()"

    if todo_pattern.search(line):
        return "TODO:s should be written as `TODO(yourname): what to do`"

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


def test_lint_line() -> None:
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
        "if let Err(err) = foo",
        "if let Err(_err) = foo",
        "if let Err(_) = foo",
        "WASM_FOO env var",
        "Wasm",
        "num_instances",
        "instances_count",
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
        "if let Err(error) = foo",
        "We use WASM in Rerun",
        "nb_instances",
    ]

    for line in should_pass:
        assert lint_line(line) is None, f'expected "{line}" to pass'

    for line in should_error:
        assert lint_line(line) is not None, f'expected "{line}" to fail'

# -----------------------------------------------------------------------------

re_declaration = re.compile(r"^\s*((pub(\(\w*\))? )?((impl|fn|struct|enum|union|trait|type)\b))")
re_attribute = re.compile(r"^\s*\#\[(error|derive)")
re_docstring = re.compile(r"^\s*///")


def is_missing_blank_line_between(prev_line: str, line: str) -> bool:
    def is_empty(line: str) -> bool:
        return (
            line == ""
            or line.startswith("#")
            or line.startswith("//")
            or line.endswith("{")
            or line.endswith("(")
            or line.endswith("\\")
            or line.endswith('r"')
            or line.endswith("]")
        )

    """Only for Rust files."""
    if re_declaration.match(line) or re_attribute.match(line) or re_docstring.match(line):
        prev_line = prev_line.strip()

        if is_empty(prev_line):
            return False

        if line.strip().endswith(";"):
            return False  # maybe a trait function

        if prev_line.endswith(",") and line.strip().startswith("impl"):
            return False

        if prev_line.strip().endswith("*"):
            return False  # maybe in a macro

        return True

    return False


def lint_vertical_spacing(lines_in: List[str]) -> Tuple[List[str], List[str]]:
    """Only for Rust files."""
    prev_line = None

    errors = []
    lines_out = []

    for line_nr, line in enumerate(lines_in):
        line_nr = line_nr + 1

        if prev_line is not None and is_missing_blank_line_between(prev_line, line):
            errors.append(f"{line_nr}: for readability, add newline before `{line.strip()}`")
            lines_out.append("\n")

        lines_out.append(line)
        prev_line = line

    return errors, lines_out


def test_lint_vertical_spacing() -> None:
    should_pass = [
        "hello world",
        """
        /// docstring
        foo

        /// docstring
        bar
        """,
        """
        trait Foo {
            fn bar();
            fn baz();
        }
        """,
        # macros:
        """
        $(#[$meta])*
        #[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
        """,
        """
        Item = (
            &PointCloudBatchInfo,
            impl Iterator<Item = &PointCloudVertex>,
        ),
        """,
    ]

    should_fail = [
        """
        /// docstring
        foo
        /// docstring
        bar
        """,
        """
        Foo,
        #[error]
        Bar,
        """,
    ]

    for code in should_pass:
        errors, _ = lint_vertical_spacing(code.split("\n"))
        assert len(errors) == 0, f"expected this to pass:\n{code}\ngot: {errors}"

    for code in should_fail:
        errors, _ = lint_vertical_spacing(code.split("\n"))
        assert len(errors) > 0, f"expected this to fail:\n{code}"

    pass

# -----------------------------------------------------------------------------


def lint_file(filepath: str, args: Any) -> int:
    with open(filepath) as f:
        lines_in = f.readlines()

    num_errors = 0

    for line_nr, line in enumerate(lines_in):
        error = lint_line(line)
        if error is not None:
            num_errors += 1
            print(f"{filepath}:{line_nr+1}: {error}")

    if filepath.endswith(".rs"):
        errors, lines_out = lint_vertical_spacing(lines_in)

        for error in errors:
            print(f"{filepath}{error}")

        if args.fix and lines_in != lines_out:
            with open(filepath, "w") as f:
                f.writelines(lines_out)
            print(f"{filepath} fixed.")

        num_errors = len(errors)

    return num_errors


def main() -> None:
    # Make sure we are bug free before we run:
    test_lint_line()
    test_lint_vertical_spacing()

    parser = argparse.ArgumentParser(description="Lint code with custom linter.")
    parser.add_argument(
        "files",
        metavar="file",
        type=str,
        nargs="*",
        help="File paths. Empty = all files, recursively.",
    )
    parser.add_argument("--fix", dest="fix", action="store_true", help="Automatically fix some problems.")

    args = parser.parse_args()

    num_errors = 0

    if args.files:
        for filepath in args.files:
            num_errors += lint_file(filepath, args)
    else:
        script_dirpath = os.path.dirname(os.path.realpath(__file__))
        root_dirpath = os.path.abspath(f"{script_dirpath}/..")
        os.chdir(root_dirpath)

        extensions = ["html", "js", "md", "py", "rs", "sh", "toml", "wgsl", "yml"]

        exclude_dirs = {"env", "venv", "target", "target_ra", "target_wasm"}

        exclude_paths = {
            "./CODE_STYLE.md",
            "./examples/rust/objectron/src/objectron.rs",  # auto-generated
            "./scripts/lint.py",  # we contain all the patterns we are linting against
            "./web_viewer/re_viewer_debug.js",  # auto-generated by wasm_bindgen
            "./web_viewer/re_viewer.js",  # auto-generated by wasm_bindgen
        }

        for root, dirs, files in os.walk(".", topdown=True):
            dirs[:] = [d for d in dirs if d not in exclude_dirs]
            for filename in files:
                extension = filename.split(".")[-1]
                if extension in extensions:
                    filepath = os.path.join(root, filename)
                    if filepath not in exclude_paths:
                        num_errors += lint_file(filepath, args)

    if num_errors == 0:
        print(f"{sys.argv[0]} finished without error")
        sys.exit(0)
    else:
        print(f"{sys.argv[0]} found {num_errors} errors.")
        sys.exit(1)


if __name__ == "__main__":
    main()
