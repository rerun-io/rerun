#!/usr/bin/env python3

"""
Runs custom linting on our code.

Adding "NOLINT" to any line makes the linter ignore that line.
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path
from typing import Any, Callable, Dict, Iterator

from ci.frontmatter import load_frontmatter
from gitignore_parser import parse_gitignore

# -----------------------------------------------------------------------------

debug_format_of_err = re.compile(r"\{\:#?\?\}.*, err")
error_match_name = re.compile(r"Err\((\w+)\)")
error_map_err_name = re.compile(r"map_err\(\|(\w+)\|")
wasm_caps = re.compile(r"\bWASM\b")
nb_prefix = re.compile(r"nb_")
else_return = re.compile(r"else\s*{\s*return;?\s*};")
explicit_quotes = re.compile(r'[^(]\\"\{\w*\}\\"')  # looks for: \"{foo}\"
ellipsis = re.compile(r"[^.]\.\.\.([^\-.0-9a-zA-Z]|$)")
ellipsis_expression = re.compile(r"[\[(<].*\.\.\..*[\])>]")
ellipsis_import = re.compile(r"from \.\.\.")
ellipsis_reference = re.compile(r"&\.\.\.")
ellipsis_bare = re.compile(r"^\s*\.\.\.\s*$")

anyhow_result = re.compile(r"Result<.*, anyhow::Error>")

double_the = re.compile(r"\bthe the\b")
double_word = re.compile(r" ([a-z]+) \1[ \.]")


Frontmatter = Dict[str, Any]


def is_valid_todo_part(part: str) -> bool:
    part = part.strip()

    if re.match(r"^[\w/-]*#\d+$", part):
        return True  # org/repo#42 or #42

    if re.match(r"^[a-z][a-z0-9_]+$", part):
        return True  # user-name

    return False


def lint_line(
    line: str, prev_line: str | None, file_extension: str = "rs", is_in_docstring: bool = False
) -> str | None:
    if line == "":
        return None

    if prev_line is None:
        prev_line_stripped = ""
    else:
        prev_line_stripped = prev_line.strip()

    if line[-1].isspace():
        return "Trailing whitespace"

    if "NOLINT" in line:
        return None  # NOLINT ignores the linter

    if file_extension not in ("py", "txt", "yaml", "yml"):
        if "Github" in line:
            return "It's 'GitHub', not 'Github'"

        if " github " in line:
            return "It's 'GitHub', not 'github'"

    if re.search(r"[.a-zA-Z]  [a-zA-Z]", line):
        return "Found double space"

    if double_the.search(line.lower()):
        return "Found 'the the'"

    if m := double_word.search(line):
        return f"Found double word: '{m.group(0)}'"

    if file_extension not in ("txt"):
        if (
            ellipsis.search(line)
            and not ellipsis_expression.search(line)
            and not ellipsis_import.search(line)
            and not ellipsis_bare.search(line)
            and not ellipsis_reference.search(line)
        ):
            return "Use … instead of ..."

    if "FIXME" in line:
        return "we prefer TODO over FIXME"

    if "HACK" in line:
        return "we prefer TODO over HACK"

    if "todo:" in line:
        return "write 'TODO:' in upper-case"

    if "todo!()" in line:
        return 'todo!() should be written as todo!("$details")'

    if m := re.search(r"TODO\(([^)]*)\)", line):
        parts = m.group(1).split(",")
        if len(parts) == 0 or not all(is_valid_todo_part(p) for p in parts):
            return "TODOs should be formatted as either TODO(name), TODO(#42) or TODO(org/repo#42)"

    if re.search(r'TODO([^_"(]|$)', line):
        return "TODO:s should be written as `TODO(yourname): what to do`"

    if "{err:?}" in line or "{err:#?}" in line or debug_format_of_err.search(line):
        return "Format errors with re_error::format or using Display - NOT Debug formatting!"

    if "from attr import dataclass" in line:
        return "Avoid 'from attr import dataclass'; prefer 'from dataclasses import dataclass'"

    if anyhow_result.search(line):
        return "Prefer using anyhow::Result<>"

    if m := re.search(error_map_err_name, line) or re.search(error_match_name, line):
        name = m.group(1)
        # if name not in ("err", "_err", "_"):
        if name in ("e", "error"):
            return "Errors should be called 'err', '_err' or '_'"

    if m := re.search(else_return, line):
        match = m.group(0)
        if match != "else { return; };":
            # Because cargo fmt doesn't handle let-else
            return f"Use 'else {{ return; }};' instead of '{match}'"

    if wasm_caps.search(line):
        return "WASM should be written 'Wasm'"

    if nb_prefix.search(line):
        return "Don't use nb_things - use num_things or thing_count instead"

    if explicit_quotes.search(line):
        return "Prefer using {:?} - it will also escape newlines etc"

    if "rec_stream" in line or "rr_stream" in line:
        return "Instantiated RecordingStreams should be named `rec`"

    if not is_in_docstring:
        if m := re.search(
            r'(RecordingStreamBuilder::new|\.init|RecordingStream)\("(\w+)',
            line,
        ) or re.search(
            r'(rr.script_setup)\(args, "(\w+)',
            line,
        ):
            app_id = m.group(2)
            if not app_id.startswith("rerun_example_"):
                return f"All examples should have an app_id starting with 'rerun_example_'. Found '{app_id}'"

    # Methods that return Self should usually be marked #[inline] or #[inline(always)] since they indicate a builder.
    if re.search(r"\(mut self.*-> Self", line):
        if prev_line_stripped != "#[inline]" and prev_line_stripped != "#[inline(always)]":
            return "Builder methods impls should be marked #[inline]"

    # Deref impls should be marked #[inline] or #[inline(always)].
    if "fn deref(&self)" in line or "fn deref_mut(&mut self)" in line:
        if prev_line_stripped != "#[inline]" and prev_line_stripped != "#[inline(always)]":
            return "Deref/DerefMut impls should be marked #[inline]"

    # Deref impls should be marked #[inline] or #[inline(always)].
    if "fn as_ref(&self)" in line or "fn borrow(&self)" in line:
        if prev_line_stripped != "#[inline]" and prev_line_stripped != "#[inline(always)]":
            return "as_ref/borrow implementations should be marked #[inline]"

    if any(s in line for s in (": &dyn std::any::Any", ": &mut dyn std::any::Any", ": &dyn Any", ": &mut dyn Any")):
        return """Functions should never take `&dyn std::any::Any` as argument since `&Box<std::any::Any>`
 itself implements `Any`, making it easy to accidentally pass the wrong object. Expect purpose defined traits instead."""

    return None


def test_lint_line() -> None:
    assert lint_line("hello world", None) is None

    should_pass = [
        "hello world",
        "todo lowercase is fine",
        'todo!("macro is ok with text")',
        "TODO_TOKEN",
        "TODO(bob):",
        "TODO(bob,alice):",
        "TODO(bob, alice):",
        "TODO(#42):",
        "TODO(#42,#43):",
        "TODO(#42, #43):",
        "TODO(n4m3/w1th-numb3r5#42)",
        "TODO(rust-lang/rust#42):",
        "TODO(rust-lang/rust#42,rust-lang/rust#43):",
        "TODO(rust-lang/rust#42, rust-lang/rust#43):",
        'eprintln!("{:?}, {err}", foo)',
        'eprintln!("{:#?}, {err}", foo)',
        'eprintln!("{err}")',
        'eprintln!("{}", err)',
        "if let Err(err) = foo",
        "if let Err(_err) = foo",
        "if let Err(_) = foo",
        "map_err(|err| …)",
        "map_err(|_err| …)",
        "map_err(|_| …)",
        "WASM_FOO env var",
        "Wasm",
        "num_instances",
        "instances_count",
        "let Some(foo) = bar else { return; };",
        "{foo:?}",
        "rec",
        "anyhow::Result<()>",
        "The theme is great",
        "template <typename... Args>",
        'protoc_prebuilt::init("22.0")',
        'rr.init("rerun_example_app")',
        """
        #[inline]
        fn foo(mut self) -> Self {
""",
        """
        #[inline(always)]
        fn foo_always(mut self) -> Self {
""",
        """
        #[inline]
        fn deref(&self) -> Self::Target {
""",
        """
        #[inline(always)]
        fn deref(&self) -> Self::Target {
""",
        """
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
""",
        """
        #[inline(always)]
        fn deref_mut(&mut self) -> &mut Self::Target {
""",
        """
        #[inline]
        fn borrow(&self) -> &Self {
""",
        """
        #[inline(always)]
        fn borrow(&self) -> &Self {
""",
        """
        #[inline]
        fn as_ref(&self) -> &Self {
""",
        """
        #[inline(always)]
        fn as_ref(&self) -> &Self {
""",
        "fn ret_any() -> &dyn std::any::Any",
        "fn ret_any_mut() -> &mut dyn std::any::Any",
    ]

    should_error = [
        "FIXME",
        "HACK",
        "TODO",
        "TODO:",
        "TODO(42)",
        "TODO(https://github.com/rerun-io/rerun/issues/42)",
        "TODO(bob/alice)",
        "TODO(bob|alice)",
        "todo!()",
        'eprintln!("{err:?}")',
        'eprintln!("{err:#?}")',
        'eprintln!("{:?}", err)',
        'eprintln!("{:#?}", err)',
        "if let Err(error) = foo",
        "map_err(|e| …)",
        "We use WASM in Rerun",
        "nb_instances",
        "inner_nb_instances",
        "let Some(foo) = bar else {return;};",
        "let Some(foo) = bar else {return};",
        "let Some(foo) = bar else { return };",
        r'println!("Problem: \"{}\"", string)',
        r'println!("Problem: \"{0}\"")',
        r'println!("Problem: \"{string}\"")',
        "trailing whitespace ",
        "rr_stream",
        "rec_stream",
        "Result<(), anyhow::Error>",
        "The the problem with double words",
        "More than meets the eye...",
        'RecordingStreamBuilder::new("missing_prefix")',
        'args.rerun.init("missing_prefix")',
        'RecordingStream("missing_prefix")',
        'rr.init("missing_prefix")',
        'rr.script_setup(args, "missing_prefix")',
        "I accidentally wrote the same same word twice",
        "fn foo(mut self) -> Self {",
        "fn deref(&self) -> Self::Target {",
        "fn deref_mut(&mut self) -> &mut Self::Target",
        "fn borrow(&self) -> &Self",
        "fn as_ref(&self) -> &Self",
        "fn take_any(thing: &dyn std::any::Any)",
        "fn take_any_mut(thing: &mut dyn std::any::Any)",
        "fn take_any(thing: &dyn Any)",
        "fn take_any_mut(thing: &mut dyn Any)",
    ]

    for test in should_pass:
        prev_line = None
        for line in test.split("\n"):
            err = lint_line(line, prev_line)
            assert err is None, f'expected "{line}" to pass, but got error: "{err}"'
            prev_line = line

    for test in should_error:
        prev_line = None
        for line in test.split("\n"):
            assert lint_line(line, prev_line) is not None, f'expected "{line}" to fail'
            prev_line = line


# -----------------------------------------------------------------------------

re_declaration = re.compile(r"^\s*((pub(\(\w*\))? )?(async )?((impl|fn|struct|enum|union|trait|type)\b))")
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
            or line.endswith('r#"')
            or line.endswith("]")
        )

    """Only for Rust files."""
    if re_declaration.match(line) or re_attribute.match(line) or re_docstring.match(line):
        line = line.strip()
        prev_line = prev_line.strip()

        if "template<" in prev_line:
            return False  # C++ template inside Rust code that generates C++ code.

        if is_empty(prev_line) or prev_line.strip().startswith("```"):
            return False

        if line.startswith("fn ") and line.endswith(";"):
            return False  # maybe a trait function

        if line.startswith("type ") and prev_line.endswith(";"):
            return False  # many type declarations in a row is fine

        if prev_line.endswith(",") and line.startswith("impl"):
            return False

        if prev_line.endswith("*"):
            return False  # maybe in a macro

        if prev_line.endswith('r##"'):
            return False  # part of a multi-line string

        return True

    return False


def lint_vertical_spacing(lines_in: list[str]) -> tuple[list[str], list[str]]:
    """Only for Rust files."""
    prev_line = None

    errors: list[str] = []
    lines_out: list[str] = []

    for line_nr, line in enumerate(lines_in):
        line_nr = line_nr + 1

        if prev_line is not None and is_missing_blank_line_between(prev_line, line):
            errors.append(f"{line_nr}: for readability, add newline before `{line.strip()}`")
            lines_out.append("\n")

        lines_out.append(line)
        prev_line = line

    return errors, lines_out


def test_lint_vertical_spacing() -> None:
    assert re_declaration.match("fn foo() {}")
    assert re_declaration.match("async fn foo() {}")
    assert re_declaration.match("pub async fn foo() {}")

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
        """
        type Response = Response<Body>;
        type Error = hyper::Error;
        """,
        """
        template<typename T>
        struct AsComponents;
        """,  # C++ template inside Rust code that generates C++ code.
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
        """
        slotmap::new_key_type! { pub struct ViewBuilderHandle; }
        type ViewBuilderMap = slotmap::SlotMap<ViewBuilderHandle, ViewBuilder>;
        """,
        """
        fn foo() {}
        fn bar() {}
        """,
        """
        async fn foo() {}
        async fn bar() {}
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


re_workspace_dep = re.compile(r"workspace\s*=\s*(true|false)")


def lint_workspace_deps(lines_in: list[str]) -> tuple[list[str], list[str]]:
    """Only for Cargo files."""

    errors = []
    lines_out = []

    for line_nr, line in enumerate(lines_in):
        line_nr = line_nr + 1

        if re_workspace_dep.search(line):
            errors.append(f"{line_nr}: Rust examples should never depend on workspace information (`{line.strip()}`)")
            lines_out.append("\n")

        lines_out.append(line)

    return errors, lines_out


def test_lint_workspace_deps() -> None:
    assert re_workspace_dep.search("workspace=true")
    assert re_workspace_dep.search("workspace=false")
    assert re_workspace_dep.search('xxx = { xxx: "yyy", workspace = true }')
    assert re_workspace_dep.search('xxx = { xxx: "yyy", workspace = false }')

    should_pass = [
        "hello world",
        """
        [package]
        name = "clock"
        version = "0.6.0-alpha.0"
        edition = "2021"
        rust-version = "1.74"
        license = "MIT OR Apache-2.0"
        publish = false

        [dependencies]
        rerun = { path = "../../../crates/rerun", features = ["web_viewer"] }

        anyhow = "1.0"
        clap = { version = "4.0", features = ["derive"] }
        glam = "0.22"
        """,
    ]

    should_fail = [
        """
        [package]
        name = "objectron"
        version.workspace = true
        edition.workspace = true
        rust-version.workspace = true
        license.workspace = true
        publish = false

        [dependencies]
        rerun = { workspace = true, features = ["web_viewer"] }

        anyhow.workspace = true
        clap = { workspace = true, features = ["derive"] }
        glam.workspace = true
        prost = "0.11"

        [build-dependencies]
        prost-build = "0.11"
        """,
    ]

    for code in should_pass:
        errors, _ = lint_workspace_deps(code.split("\n"))
        assert len(errors) == 0, f"expected this to pass:\n{code}\ngot: {errors}"

    for code in should_fail:
        errors, _ = lint_workspace_deps(code.split("\n"))
        assert len(errors) > 0, f"expected this to fail:\n{code}"

    pass


# -----------------------------------------------------------------------------
# We may not use egui's widgets for which we have a custom version in re_ui.

# Note: this really is best-effort detection, it will only catch the most common code layout cases. If this gets any
# more complicated, a syn-based linter in Rust would certainly be better approach.

re_forbidden_widgets = [
    (
        re.compile(r"(?<!\w)ui[\t ]*(//.*)?\s*.\s*checkbox(?!\w)", re.MULTILINE),
        "ui.checkbox() is forbidden (use re_ui.checkbox() instead)",
    ),
    (
        re.compile(r"(?<!\w)ui[\t ]*(//.*)?\s*.\s*radio_value(?!\w)", re.MULTILINE),
        "ui.radio_value() is forbidden (use re_ui.radio_value() instead)",
    ),
]


def lint_forbidden_widgets(content: str) -> Iterator[tuple[str, int, int]]:
    for re_widget, error in re_forbidden_widgets:
        for match in re_widget.finditer(content):
            yield error, match.start(0), match.end(0)


def test_lint_forbidden_widgets() -> None:
    re_checkbox = re_forbidden_widgets[0][0]
    assert re_checkbox.search("ui.checkbox")
    assert re_checkbox.search("  ui.\n\t\t   checkbox  ")
    assert re_checkbox.search("  ui.\n\t\t   checkbox()")
    assert re_checkbox.search("  ui\n\t\t   .checkbox()")
    assert re_checkbox.search("  ui //bla\n\t\t   .checkbox()")
    assert not re_checkbox.search("re_ui.checkbox")
    assert not re_checkbox.search("ui.checkbox_re")

    should_fail_two_times = """
        ui.checkbox()
        re_ui.checkbox()
        ui.checkbox_re()

        ui  // hello!
            .checkbox()
            .bla()
    """

    res = list(lint_forbidden_widgets(should_fail_two_times))
    assert len(res) == 2
    assert _index_to_line_nr(should_fail_two_times, res[0][1]) == 1
    assert _index_to_line_nr(should_fail_two_times, res[1][1]) == 5


def lint_example_description(filepath: str, fm: Frontmatter) -> list[str]:
    # only applies to examples' readme

    if not filepath.startswith("./examples/python") or not filepath.endswith("README.md"):
        return []

    desc = fm.get("description", "")
    if len(desc) > 130:
        return [f"Frontmatter: description is too long ({len(desc)} > 130)"]
    else:
        return []


def lint_frontmatter(filepath: str, content: str) -> list[str]:
    """Only for Markdown files."""

    errors: list[str] = []
    if not filepath.endswith(".md"):
        return errors

    fm = load_frontmatter(content)
    if fm is None:
        return []

    errors += lint_example_description(filepath, fm)

    # TODO(ab): check for missing fields (when descriptions are populated everywhere)

    return errors


# -----------------------------------------------------------------------------


def _index_to_line_nr(content: str, index: int) -> int:
    """Converts a 0-based index into a 0-based line number."""
    return content[:index].count("\n")


class SourceFile:
    """Wrapper over a source file with some utility functions."""

    def __init__(self, path: str):
        self.path = os.path.abspath(path)
        self.ext = path.split(".")[-1]
        with open(path, encoding="utf8") as f:
            self.lines = f.readlines()
        self._update_content()

    def _update_content(self) -> None:
        """Sync everything with `self.lines`."""
        self.content = "".join(self.lines)

        # gather lines with a `NOLINT` marker
        self.no_lints = {i for i, line in enumerate(self.lines) if "NOLINT" in line}

    def rewrite(self, new_lines: list[str]) -> None:
        """Rewrite the contents of the file."""
        if new_lines != self.lines:
            self.lines = new_lines
            with open(self.path, "w") as f:
                f.writelines(new_lines)
            self._update_content()
            print(f"{self.path} fixed.")

    def should_ignore(self, from_line: int, to_line: int | None = None) -> bool:
        """
        Determines if we should ignore a violation.

        NOLINT might be on the same line(s) as the violation or the previous line.
        """

        if to_line is None:
            to_line = from_line
        return any(i in self.no_lints for i in range(from_line - 1, to_line + 1))

    def should_ignore_index(self, start_idx: int, end_idx: int | None = None) -> bool:
        """Same as `should_ignore` but takes 0-based indices instead of line numbers."""
        return self.should_ignore(
            _index_to_line_nr(self.content, start_idx),
            _index_to_line_nr(self.content, end_idx) if end_idx is not None else None,
        )

    def error(self, message: str, *, line_nr: int | None = None, index: int | None = None) -> str:
        """Construct an error message. If either `line_nr` or `index` is passed, it's used to indicate a line number."""
        if line_nr is None and index is not None:
            line_nr = _index_to_line_nr(self.content, index)
        if line_nr is None:
            return f"{self.path}:{message}"
        else:
            return f"{self.path}:{line_nr+1}: {message}"


def lint_file(filepath: str, args: Any) -> int:
    source = SourceFile(filepath)
    num_errors = 0

    error: str | None

    is_in_docstring = False

    prev_line = None
    for line_nr, line in enumerate(source.lines):
        if line == "" or line[-1] != "\n":
            error = "Missing newline at end of file"
        else:
            line = line[:-1]
            if line.strip() == '"""':
                is_in_docstring = not is_in_docstring
            error = lint_line(line, prev_line, source.ext, is_in_docstring)
            prev_line = line
        if error is not None:
            num_errors += 1
            print(source.error(error, line_nr=line_nr))

    if filepath.endswith(".hpp"):
        if not any(line.startswith("#pragma once") for line in source.lines):
            print(source.error("Missing `#pragma once` in C++ header file"))
            num_errors += 1

    if filepath.endswith(".rs") or filepath.endswith(".fbs"):
        if filepath.endswith(".rs"):
            for error, start_idx, end_idx in lint_forbidden_widgets(source.content):
                if not source.should_ignore_index(start_idx, end_idx):
                    print(source.error(error, index=start_idx))
                    num_errors += 1

        errors, lines_out = lint_vertical_spacing(source.lines)
        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

        if args.fix:
            source.rewrite(lines_out)

    if filepath.startswith("./examples/rust") and filepath.endswith("Cargo.toml"):
        errors, lines_out = lint_workspace_deps(source.lines)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

        if args.fix:
            source.rewrite(lines_out)

    # Markdown-specific lints
    if filepath.endswith(".md"):
        errors = lint_frontmatter(filepath, source.content)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

    return num_errors


def lint_crate_docs(should_ignore: Callable[[Any], bool]) -> int:
    """Make sure ARCHITECTURE.md talks about every single crate we have."""

    crates_dir = Path("crates")
    architecture_md_file = Path("ARCHITECTURE.md")

    architecture_md = architecture_md_file.read_text()

    # extract all crate names ("re_...") from ARCHITECTURE.md to ensure they actually exist
    listed_crates: dict[str, int] = {}
    for i, line in enumerate(architecture_md.split("\n"), start=1):
        for crate_name in re.findall(r"\bre_\w+", line):
            if crate_name not in listed_crates:
                listed_crates[crate_name] = i

    error_count = 0
    for cargo_toml in crates_dir.glob("**/Cargo.toml"):
        crate = cargo_toml.parent
        crate_name = crate.name

        if crate_name in listed_crates:
            del listed_crates[crate_name]

        if should_ignore(crate):
            continue

        if not re.search(r"\b" + crate_name + r"\b", architecture_md):
            print(f"{architecture_md_file}: missing documentation for crate {crate.name}")
            error_count += 1

    for crate_name, line_nr in sorted(listed_crates.items(), key=lambda x: x[1]):
        print(f"{architecture_md_file}:{line_nr}: crate name {crate_name} does not exist")
        error_count += 1

    return error_count


def main() -> None:
    # Make sure we are bug free before we run:
    test_lint_line()
    test_lint_vertical_spacing()
    test_lint_workspace_deps()

    parser = argparse.ArgumentParser(description="Lint code with custom linter.")
    parser.add_argument(
        "files",
        metavar="file",
        type=str,
        nargs="*",
        help="File paths. Empty = all files, recursively.",
    )
    parser.add_argument(
        "--fix",
        dest="fix",
        action="store_true",
        help="Automatically fix some problems.",
    )

    args = parser.parse_args()

    num_errors = 0

    # This list of file extensions matches the one in `.github/workflows/documentation.yaml`
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
        "yaml",
        "yml",
    ]

    exclude_paths = (
        "./.github/workflows/reusable_checks.yml",  # zombie TODO hunting job
        "./CODE_STYLE.md",
        "./crates/re_types_builder/src/reflection.rs",  # auto-generated
        "./examples/assets",
        "./examples/python/detect_and_track_objects/cache/version.txt",
        "./examples/python/objectron/proto/",  # auto-generated
        "./examples/rust/objectron/src/objectron.rs",  # auto-generated
        "./rerun_cpp/docs/doxygen-awesome/",  # copied from an external repository
        "./rerun_cpp/docs/html",
        "./rerun_cpp/src/rerun/c/arrow_c_data_interface.h",  # Not our code
        "./rerun_cpp/src/rerun/third_party/cxxopts.hpp",  # vendored
        "./rerun_py/site/",  # is in `.gitignore` which this script doesn't fully respect
        "./scripts/lint.py",  # we contain all the patterns we are linting against
        "./scripts/zombie_todos.py",
        "./tests/python/release_checklist/main.py",
        "./web_viewer/re_viewer.js",  # auto-generated by wasm_bindgen
    )

    should_ignore = parse_gitignore(".gitignore")  # TODO(emilk): parse all .gitignore files, not just top-level

    script_dirpath = os.path.dirname(os.path.realpath(__file__))
    root_dirpath = os.path.abspath(f"{script_dirpath}/..")
    os.chdir(root_dirpath)

    if args.files:
        for filepath in args.files:
            filepath = os.path.join(".", os.path.relpath(filepath, root_dirpath))
            filepath = str(filepath).replace("\\", "/")
            extension = filepath.split(".")[-1]
            if extension in extensions:
                if should_ignore(filepath) or filepath.startswith(exclude_paths):
                    continue
                num_errors += lint_file(filepath, args)
    else:
        for root, dirs, files in os.walk(".", topdown=True):
            dirs[:] = [d for d in dirs if not should_ignore(d)]

            for filename in files:
                extension = filename.split(".")[-1]
                if extension in extensions:
                    filepath = os.path.join(root, filename)
                    filepath = os.path.join(".", os.path.relpath(filepath, root_dirpath))
                    filepath = str(filepath).replace("\\", "/")
                    if should_ignore(filepath) or filepath.startswith(exclude_paths):
                        continue
                    num_errors += lint_file(filepath, args)

        # Since no files have been specified, we also run the global lints.
        num_errors += lint_crate_docs(should_ignore)

    if num_errors == 0:
        print(f"{sys.argv[0]} finished without error")
        sys.exit(0)
    else:
        print(f"{sys.argv[0]} found {num_errors} errors.")
        sys.exit(1)


if __name__ == "__main__":
    main()
