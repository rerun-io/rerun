#!/usr/bin/env python3

"""
Runs custom linting on our code.

Adding "NOLINT" to any line makes the linter ignore that line. Adding a pair of "NOLINT_START" and "NOLINT_END" makes
the linter ignore these lines, as well as all lines in between.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path
from typing import Any

import git
from ci.frontmatter import load_frontmatter
from ci.validate_attrs_init_calls import AttrsInitValidator
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
ellipsis_expression = re.compile(r"[\[\]\(\)<>\{\}]?.*\.\.\..*[\[\]\(\)<>\{\}]")
ellipsis_import = re.compile(r"from \.\.\.")
ellipsis_reference = re.compile(r"&\.\.\.")
ellipsis_bare = re.compile(r"^\s*\.\.\.\s*$")

anyhow_result = re.compile(r"Result<.*, anyhow::Error>")
pyclass_start = re.compile(r"#\[pyclass\(")
pymethods_start = re.compile(r"#\[pymethods\]")

double_the = re.compile(r"\bthe the\b")
double_word = re.compile(r" ([a-z]+) \1[ \.]")

Frontmatter = dict[str, Any]


def is_valid_todo_part(part: str) -> bool:
    part = part.strip()

    if re.match(r"^[\w/-]*#\d+$", part):
        return True  # org/repo#42 or #42

    if re.match(r"^[a-z][a-z0-9_]+$", part):
        return True  # user-name

    if re.match(r"^RR-\d+$", part):
        return True  # linear issue

    return False


def check_string(s: str) -> str | None:
    """Check that the string has the correct casing."""
    if len(s) == 0:
        return None

    bad_titles = [
        "Blueprint",
        "Class",
        "Container",
        "Entity",
        "EntityPath",
        "Epoch",
        "Instance",
        "Path",
        "Recording",
        "Result",
        "Space",
        "Store",
        "View",
        "Viewport",
    ]

    if m := re.search(r"[^.] ([A-Z]\w+)", s):
        word = m.group(1)
        if word in bad_titles:
            return f"Do not use title casing ({word}). See https://github.com/rerun-io/rerun/blob/main/DESIGN.md"

    return None


def lint_url(url: str) -> str | None:
    ALLOW_LIST_URLS = {
        "https://github.com/lycheeverse/lychee/blob/master/lychee.example.toml",
        "https://github.com/rerun-io/documentation/blob/main/src/utils/tokens.ts",
        "https://github.com/rerun-io/landing/blob/main/src/lib/lang.ts",  # if this file moves we should check the linked code.
        "https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md",
        "https://github.com/rerun-io/rerun/blob/main/CODE_OF_CONDUCT.md",
        "https://github.com/rerun-io/rerun/blob/main/CONTRIBUTING.md",
        "https://github.com/rerun-io/rerun/blob/main/LICENSE-APACHE",
        "https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT",
    }

    if url in ALLOW_LIST_URLS:
        return None

    if m := re.match(r"https://github.com/.*/blob/(\w+)/.*", url):
        branch = m.group(1)
        if branch in ("main", "master", "trunk", "latest"):
            if "#L" in url:
                return f"Do not link directly to a file:line on '{branch}' - it may change! Use a perma-link instead (commit hash or tag). Url: {url}"

            if "/README.md" in url:
                pass  # Probably fine
            elif url.startswith("https://github.com/rerun-io/rerun/blob/"):
                pass  # TODO(#6077): figure out how we best link to our own code from our docs
            else:
                return f"Do not link directly to a file on '{branch}' - it may disappear! Use a commit hash or tag instead. Url: {url}"

    return None


def lint_line(
    line: str,
    prev_line: str | None,
    file_extension: str = "rs",
    is_in_docstring: bool = False,
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
        if r"\n  " not in line:  # Allow `\n  `, which happens e.g. when markdown is embedded in a string
            return "Found double space"

    if double_the.search(line.lower()):
        return "Found 'the the'"

    if m := double_word.search(line):
        return f"Found double word: '{m.group(0)}'"

    if m := re.search(r'https?://[^ )"]+', line):
        url = m.group(0)
        if err := lint_url(url):
            return err

    if file_extension != "":
        # We lint against writing ellipsis using three dots for the sake of our UI:
        # * We want it consistent
        # * We want it beautiful (`…` looks different from `...`)
        # * We don't want linebreaks in the middle of an ellipsis
        #
        # This lint is therefore most important in user-facing code, such as the UI,
        # but we also care about beautiful docs, so at the moment this lint is quite "inclusive".
        if ellipsis.search(line):
            has_quote = '"' in line or "'" in line
            if (has_quote and "Callable" not in line) or (
                file_extension not in "py"
                and not ellipsis_expression.search(line)
                and not ellipsis_import.search(line)
                and not ellipsis_bare.search(line)
                and not ellipsis_reference.search(line)
            ):
                return "Use … instead of ... (on Mac it's option+;)"

    if "http" not in line:
        if re.search(r"\b2d\b", line):
            return "we prefer '2D' over '2d'"
        if re.search(r"\b3d\b", line):
            return "we prefer '3D' over '3d'"

    if (
        "recording=rec" in line
        and "rr." not in line
        and "recording=rec.to_native()" not in line
        and "recording=recording.to_native()" not in line
    ):
        return "you must cast the RecordingStream first: `recording=recording.to_native()"

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

    if m := re.search(r'"([^"]*)"', line):
        if err := check_string(m.group(1)):
            return err

    if "rec_stream" in line or "rr_stream" in line:
        return "Instantiated RecordingStreams should be named `rec`"

    if not is_in_docstring:
        if m := re.search(
            r'(RecordingStreamBuilder::new|\.init|RecordingStream)\("([^"]*)',
            line,
        ) or re.search(
            r'(rr.script_setup)\(args, "(\w*)',
            line,
        ):
            app_id = m.group(2)
            if not app_id.startswith("rerun_example_") and not app_id == "<your_app_name>":
                return f"All examples should have an app_id starting with 'rerun_example_'. Found '{app_id}'"

    # Deref impls should be marked #[inline] or #[inline(always)].
    if "fn deref(&self)" in line or "fn deref_mut(&mut self)" in line:
        if prev_line_stripped not in {"#[inline]", "#[inline(always)]"}:
            return "Deref/DerefMut impls should be marked #[inline]"

    # Deref impls should be marked #[inline] or #[inline(always)].
    if "fn as_ref(&self)" in line or "fn borrow(&self)" in line:
        if prev_line_stripped not in {"#[inline]", "#[inline(always)]"}:
            return "as_ref/borrow implementations should be marked #[inline]"

    if any(
        s in line
        for s in (
            ": &dyn std::any::Any",
            ": &mut dyn std::any::Any",
            ": &dyn Any",
            ": &mut dyn Any",
        )
    ):
        return """Functions should never take `&dyn std::any::Any` as argument since `&Box<std::any::Any>`
 itself implements `Any`, making it easy to accidentally pass the wrong object. Expect purpose defined traits instead."""

    return None


def test_lint_line() -> None:
    assert lint_line("hello world", None) is None

    should_pass = [
        "hello world",
        "this is a 2D view",
        "todo lowercase is fine",
        'todo!("Macro is ok with text")',
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
        'ui.label("This is fine. Correct casing.")',
        "rec",
        "anyhow::Result<()>",
        "The theme is great",
        "template <typename... Args>",
        '_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])',
        'protoc_prebuilt::init("22.0")',
        'rr.init("rerun_example_app")',
        'rr.script_setup(args, "rerun_example_app")',
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
        "this is a 2d view",
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
        'ui.label("This uses ugly title casing for View.")',
        "trailing whitespace ",
        "rr_stream",
        "rec_stream",
        "Result<(), anyhow::Error>",
        "The the problem with double words",
        "More than meets the eye...",
        're_log::trace!("Performing migrations...");',
        'rr.log("/", rr.TextLog("Logging things..."))',
        'logging.info("Detection finished...")',
        'RecordingStreamBuilder::new("missing_prefix")',
        'args.rerun.init("missing_prefix")',
        'RecordingStream("missing_prefix")',
        'rr.init("missing_prefix")',
        'rr.script_setup(args, "missing_prefix")',
        'rr.script_setup(args, "")',
        "I accidentally wrote the same same word twice",
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
re_attribute = re.compile(r"^\s*\#\[(error|derive|inline)")
re_docstring = re.compile(r"^\s*///")


def is_missing_blank_line_between(prev_line: str, line: str) -> bool:
    def is_empty(line: str) -> bool:
        return line == "" or line.startswith(("#", "//")) or line.endswith(("{", "(", "\\", 'r"', 'r#"', "]"))

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


# -----------------------------------------------------------------------------


workspace_lints = re.compile(r"\[lints\]\nworkspace\s*=\s*true")


def lint_workspace_lints(cargo_file_content: str) -> str | None:
    """Checks that a non-example cargo file has a lints section that sets workspace to true."""

    if workspace_lints.search(cargo_file_content):
        return None
    else:
        return "Non-example cargo files should have a [lints] section with workspace = true"


# -----------------------------------------------------------------------------


def lint_pyclass_requirements(lines_in: list[str]) -> tuple[list[str], list[int], list[str]]:
    """Only for Rust files. Check that #[pyclass(...)] declarations include 'eq' and the correct module."""

    errors: list[str] = []
    error_linenumbers: list[int] = []
    error_codes: list[str] = []

    i = 0
    while i < len(lines_in):
        line = lines_in[i]
        line_nr = i + 1

        # Check if this line starts a pyclass declaration
        if pyclass_start.search(line.strip()):
            # Collect the entire pyclass declaration (it might span multiple lines)
            pyclass_content = line
            original_line_nr = line_nr

            # Keep reading lines until we find the closing parenthesis
            paren_count = line.count("(") - line.count(")")
            j = i + 1

            while paren_count > 0 and j < len(lines_in):
                next_line = lines_in[j]
                pyclass_content += next_line
                paren_count += next_line.count("(") - next_line.count(")")
                j += 1

            # Check if 'eq' is present in the pyclass declaration
            # Look for 'eq' as a standalone parameter (not part of another word)
            # First remove comments to avoid false matches in comments
            pyclass_content_no_comments = re.sub(r"//.*", "", pyclass_content)
            if not re.search(r"\beq\b", pyclass_content_no_comments):
                errors.append(
                    f"{original_line_nr}: #[pyclass(...)] should include 'eq' parameter for Python equality support"
                )
                error_linenumbers.append(original_line_nr)
                error_codes.append("py-cls-eq")

            # Check if the correct module is specified
            expected_module = 'module = "rerun_bindings.rerun_bindings"'
            if expected_module not in pyclass_content:
                errors.append(
                    f"{original_line_nr}: #[pyclass(...)] should include 'module = \"rerun_bindings.rerun_bindings\"' parameter"
                )
                error_linenumbers.append(original_line_nr)
                error_codes.append("py-cls-mod")

            # Move the index to after the pyclass declaration
            i = j
        else:
            i += 1

    return errors, error_linenumbers, error_codes


def lint_pymethods_requirements(lines_in: list[str]) -> tuple[list[str], list[int], list[str]]:
    """Only for Rust files. Check that #[pymethods] blocks have a __str__ method."""

    errors: list[str] = []
    error_linenumbers: list[int] = []
    error_codes: list[str] = []

    i = 0
    while i < len(lines_in):
        line = lines_in[i]
        line_nr = i + 1

        # Check if this line starts a pymethods declaration
        if pymethods_start.search(line.strip()):
            # Find the corresponding impl block
            j = i + 1
            impl_start_line = None
            class_name = None

            # Look for the impl block that follows
            while j < len(lines_in):
                impl_line = lines_in[j].strip()
                if impl_line.startswith("impl "):
                    impl_start_line = j
                    # Extract class name from "impl ClassName {"
                    match = re.search(r"impl\s+(\w+)\s*\{", impl_line)
                    if match:
                        class_name = match.group(1)
                    break
                elif impl_line and not impl_line.startswith("//"):
                    # If we hit any non-comment, non-empty line that's not impl, stop looking
                    break
                j += 1

            if impl_start_line is None or class_name is None:
                i += 1
                continue

            # Find the end of the impl block by counting braces
            brace_count = 0
            impl_content = ""
            k = impl_start_line

            while k < len(lines_in):
                current_line = lines_in[k]
                impl_content += current_line
                brace_count += current_line.count("{") - current_line.count("}")

                if brace_count == 0 and "{" in lines_in[impl_start_line]:
                    # We've found the end of the impl block
                    break
                k += 1

            # Check if __str__ or __repr__ is present in the impl content
            # Remove comments to avoid false matches
            impl_content_no_comments = re.sub(r"//.*", "", impl_content)

            has_str = re.search(r"\b__str__\b", impl_content_no_comments)
            has_repr = re.search(r"\b__repr__\b", impl_content_no_comments)

            if not has_str and not has_repr:
                errors.append(
                    f"{line_nr}: #[pymethods] impl {class_name} should include a '__str__' method (or '__repr__' which serves as fallback)"
                )
                error_linenumbers.append(line_nr)
                error_codes.append("py-mthd-str")

            # Move the index to after the impl block
            i = k + 1
        else:
            i += 1

    return errors, error_linenumbers, error_codes


def test_lint_pymethods_requirements() -> None:
    """Test the lint_pymethods_requirements function with various pymethods declarations."""

    should_pass = [
        # pymethods with __str__
        """#[pymethods]
impl MyClass {
    pub fn __str__(&self) -> String {
        "test".to_string()
    }
}""",
        # pymethods with __repr__ (serves as __str__ fallback)
        """#[pymethods]
impl MyClass {
    fn __repr__(&self) -> String {
        "test".to_string()
    }
}""",
        # pymethods with both __str__ and __repr__
        """#[pymethods]
impl MyClass {
    pub fn __str__(&self) -> String {
        "str".to_string()
    }
    fn __repr__(&self) -> String {
        "repr".to_string()
    }
}""",
        # pymethods with other methods and __str__
        """#[pymethods]
impl MyClass {
    #[new]
    pub fn new() -> Self {
        Self {}
    }
    pub fn __str__(&self) -> String {
        "test".to_string()
    }
}""",
    ]

    should_error = [
        # pymethods without __str__ or __repr__
        """#[pymethods]
impl MyClass {
    #[new]
    pub fn new() -> Self {
        Self {}
    }
}""",
        # pymethods with other methods but no __str__ or __repr__
        """#[pymethods]
impl MyClass {
    pub fn other_method(&self) -> i32 {
        42
    }
    pub fn another_method(&self) -> bool {
        true
    }
}""",
    ]

    # Test cases that should pass (no errors)
    for test_case in should_pass:
        lines = test_case.split("\n")
        errors, _, _ = lint_pymethods_requirements(lines)
        assert len(errors) == 0, f'expected "{test_case}" to pass, but got errors: {errors}'

    # Test cases that should fail (produce errors)
    for test_case in should_error:
        lines = test_case.split("\n")
        errors, _, _ = lint_pymethods_requirements(lines)
        assert len(errors) > 0, f'expected "{test_case}" to fail, but got no errors'


def test_lint_pyclass_requirements() -> None:
    """Test the lint_pyclass_requirements function with various pyclass declarations."""

    should_pass = [
        # Simple pyclass with eq and module
        '#[pyclass(eq, module = "rerun_bindings.rerun_bindings")]',
        # Multiple parameters including eq and module
        '#[pyclass(frozen, eq, hash, module = "rerun_bindings.rerun_bindings")]',
        # eq in different position
        '#[pyclass(eq, frozen, module = "rerun_bindings.rerun_bindings")]',
        # Multi-line pyclass with eq and module
        '#[pyclass(\n    frozen,\n    eq,\n    hash,\n    module = "rerun_bindings.rerun_bindings"\n)]',
        # eq at the end
        '#[pyclass(frozen, hash, eq, module = "rerun_bindings.rerun_bindings")]',
        # With module specification and eq
        '#[pyclass(eq, module = "rerun_bindings.rerun_bindings")]',
        # Complex real-world example
        """#[pyclass(
            frozen,
            eq,
            hash,
            name = "IndexColumnDescriptor",
            module = "rerun_bindings.rerun_bindings"
        )]""",
        # With name parameter
        '#[pyclass(eq, name = "MyClass", module = "rerun_bindings.rerun_bindings")]',
    ]

    should_error = [
        # Missing eq parameter
        '#[pyclass(frozen, module = "rerun_bindings.rerun_bindings")]',
        # Multiple parameters but no eq
        '#[pyclass(frozen, hash, module = "rerun_bindings.rerun_bindings")]',
        # With module but no eq
        '#[pyclass(module = "rerun_bindings.rerun_bindings")]',
        # With eq but no module
        "#[pyclass(eq, frozen)]",
        # Missing both eq and module
        "#[pyclass(frozen)]",
        # Multi-line without eq
        '#[pyclass(\n    frozen,\n    hash,\n    module = "rerun_bindings.rerun_bindings"\n)]',
        # Multi-line without module
        "#[pyclass(\n    frozen,\n    eq,\n    hash\n)]",
        # Complex example without eq
        """#[pyclass(
            frozen,
            hash,
            name = "IndexColumnDescriptor",
            module = "rerun_bindings.rerun_bindings"
        )]""",
        # Complex example without module
        """#[pyclass(
            frozen,
            eq,
            hash,
            name = "IndexColumnDescriptor"
        )]""",
        # Wrong module name
        '#[pyclass(eq, module = "wrong_module")]',
    ]

    # Test cases that should pass (no errors)
    for test_case in should_pass:
        lines = test_case.split("\n")
        errors, _, _ = lint_pyclass_requirements(lines)
        assert len(errors) == 0, f'expected "{test_case}" to pass, but got errors: {errors}'

    # Test cases that should fail (produce errors)
    for test_case in should_error:
        lines = test_case.split("\n")
        errors, _, _ = lint_pyclass_requirements(lines)
        assert len(errors) > 0, f'expected "{test_case}" to fail, but got no errors'


# -----------------------------------------------------------------------------

force_capitalized = [
    "2D",
    "3D",
    "Apache",
    "API",
    "APIs",
    "April",
    "Bevy",
    "C",
    "C++",
    "C++17,",  # easier than coding up a special case
    "CI",
    "Colab",
    "Google",
    "Gradio",
    "gRPC",
    "GUI",
    "GUIs",
    "Intel",
    "July",
    "Jupyter",
    "LeRobot",
    "Linux",
    "Mac",
    "macOS",
    "Macs",
    "ML",
    "Numpy",
    "nuScenes",
    "Pandas",
    "PDF",
    "Pixi",
    "Polars",
    "Python",
    "Q1",
    "Q2",
    "Q3",
    "Q4",
    "Rerun",
    "Rust",
    "SAM",
    "SDK",
    "SDKs",
    "UI",
    "UIs",
    "UX",
    "Wasm",
    # "Arrow",   # Would be nice to capitalize in the right context, but it's a too common word.
    # "Windows", # Consider "multiple plot windows"
]

allow_capitalized = [
    "Viewer",
    # Referring to the Rerun Viewer as just "the Viewer" is fine, but not all mentions of "viewer" are capitalized.
    "Arrow",
    # Referring to the Apache Arrow project as just "Arrow" is fine, but not all mentions of "arrow" are capitalized.
]

force_capitalized_as_lower = [word.lower() for word in force_capitalized]
allow_capitalized_as_lower = [word.lower() for word in allow_capitalized]


def split_words(input_string: str) -> list[str]:
    result = []
    word = ""
    for char in input_string:
        if char.isalpha() or char.isdigit() or char in "/_@`.!?+-()":
            word += char
        else:
            if word:
                result.append(word)
                word = ""
            result.append(char)
    if word:
        result.append(word)
    return result


def is_emoji(s: str) -> bool:
    """Returns true if the string contains an emoji."""
    # Written by Copilot
    return any(
        0x1F600 <= ord(c) <= 0x1F64F  # Emoticons
        or 0x1F300 <= ord(c) <= 0x1F5FF  # Miscellaneous Symbols and Pictographs
        or 0x1F680 <= ord(c) <= 0x1F6FF  # Transport and Map Symbols
        or 0x2600 <= ord(c) <= 0x26FF  # Miscellaneous Symbols
        or 0x2700 <= ord(c) <= 0x27BF  # Dingbats
        or 0xFE00 <= ord(c) <= 0xFE0F  # Variation Selectors
        or 0x1F900 <= ord(c) <= 0x1F9FF  # Supplemental Symbols and Pictographs
        or 0x1FA70 <= ord(c) <= 0x1FAFF  # Symbols and Pictographs Extended-A
        for c in s
    )


def test_is_emoji() -> None:
    assert not is_emoji("A")
    assert not is_emoji("Ö")
    assert is_emoji("😀")
    assert is_emoji("⚠️")


def test_split_words() -> None:
    test_cases = [
        ("hello world", ["hello", " ", "world"]),
        ("hello foo@rerun.io", ["hello", " ", "foo@rerun.io"]),
        ("www.rerun.io", ["www.rerun.io"]),
        ("`rerun`", ["`rerun`"]),
    ]

    for input, expected in test_cases:
        actual = split_words(input)
        assert actual == expected, f"Expected '{input}' to split into {expected}, got {actual}"


def fix_header_casing(s: str) -> str:
    def is_acronym_or_pascal_case(s: str) -> bool:
        return sum(1 for c in s if c.isupper()) > 1

    if s.startswith("["):
        return s  # We don't handle links in headers, yet

    new_words: list[str] = []
    last_punctuation = None
    inline_code_block = False
    is_first_word = True

    words = s.strip().split(" ")

    for word in words:
        if word == "":
            continue

        if word == "I":
            new_words.append(word)
            continue

        if is_emoji(word):
            new_words.append(word)
            continue

        if word.startswith("`"):
            inline_code_block = True

        if last_punctuation:
            word = word.capitalize()
            last_punctuation = None
        elif not inline_code_block and not word.startswith("`") and not word.startswith('"'):
            try:
                idx = force_capitalized_as_lower.index(word.lower())
            except ValueError:
                idx = None

            if word.endswith(("?", "!", ".")):
                last_punctuation = word[-1]
                word = word[:-1]
            elif idx is not None:
                word = force_capitalized[idx]
            elif is_acronym_or_pascal_case(word) or any(c in ("_", "(", ".") for c in word):
                pass  # acroym, PascalCase, code, …
            elif word.lower() in allow_capitalized_as_lower:
                pass
            elif is_first_word:
                word = word.capitalize()
            else:
                word = word.lower()

        if word.endswith("`"):
            inline_code_block = False

        new_words.append((word + last_punctuation) if last_punctuation else word)
        is_first_word = False

    return " ".join(new_words)


def fix_enforced_upper_case(s: str) -> str:
    new_words: list[str] = []
    inline_code_block = False

    for word in split_words(s):
        if word.startswith("`"):
            inline_code_block = True
        if word.endswith("`"):
            inline_code_block = False

        if word.strip() != "" and not inline_code_block and not word.startswith("`"):
            try:
                idx = force_capitalized_as_lower.index(word.lower())
                word = force_capitalized[idx]
            except ValueError:
                pass

        new_words.append(word)

    return "".join(new_words)


def lint_markdown(filepath: str, source: SourceFile) -> tuple[list[str], list[str]]:
    """Only for .md files."""

    errors: list[str] = []
    lines_out: list[str] = []

    in_example_readme = (
        "/examples/python/" in filepath
        and filepath.endswith("README.md")
        and not filepath.endswith("/examples/python/README.md")
    )
    in_code_of_conduct = filepath.endswith("CODE_OF_CONDUCT.md")

    if in_code_of_conduct:
        return errors, source.lines

    in_code_block = False
    in_frontmatter = False
    in_metadata = False
    for line_nr, line in enumerate(source.lines):
        line_nr = line_nr + 1

        if line.strip().startswith("```"):
            in_code_block = not in_code_block

        if line.startswith("---"):
            in_frontmatter = not in_frontmatter
        if line.startswith("<!--[metadata]"):
            in_metadata = True
        if in_metadata and line.startswith("-->"):
            in_metadata = False

        if not in_code_block and not source.should_ignore(line_nr):
            if not in_metadata:
                # Check the casing on markdown headers
                if m := re.match(r"(\#+ )(.*)", line):
                    new_header = fix_header_casing(m.group(2))
                    if new_header != m.group(2):
                        errors.append(
                            f"{line_nr}: Markdown headers should NOT be title cased, except certain words which are always capitalized. This should be '{new_header}'.",
                        )
                        line = m.group(1) + new_header + "\n"

            # Check the casing on `title = "…"` frontmatter
            elif m := re.match(r'title\s*\=\s*"(.*)"', line):
                new_title = fix_header_casing(m.group(1))
                if new_title != m.group(1):
                    errors.append(
                        f"{line_nr}: Titles should NOT be title cased, except certain words which are always capitalized. This should be '{new_title}'.",
                    )
                    line = f'title = "{new_title}"\n'

            # Enforce capitalization on certain words in the main text.
            elif not in_frontmatter:
                new_line = fix_enforced_upper_case(line)
                if new_line != line:
                    errors.append(f"{line_nr}: Certain words should be capitalized. This should be '{new_line}'.")
                    line = new_line

            if in_example_readme and not in_metadata:
                # Check that <h1> is not used in example READMEs
                if line.startswith("#") and not line.startswith("##"):
                    errors.append(
                        f"{line_nr}: Do not use top-level headers in example READMEs, they are reserved for page title.",
                    )

        lines_out.append(line)

    return errors, lines_out


def lint_example_description(filepath: str) -> list[str]:
    # only applies to examples' readme

    if not filepath.startswith("./examples/python") or not filepath.endswith("README.md"):
        return []

    return []


def lint_frontmatter(filepath: str, content: str) -> list[str]:
    """Only for Markdown files."""

    errors: list[str] = []
    if not filepath.endswith(".md"):
        return errors

    try:
        load_frontmatter(content)
    except Exception as e:
        errors.append(f"Error parsing frontmatter: {e}")

    errors += lint_example_description(filepath)

    return errors


# -----------------------------------------------------------------------------


def lint_attrs_init_completeness(filepath: str) -> list[str]:
    """Check that __attrs_init__ calls in extension files include all available arguments."""

    try:
        # Use the validator directly instead of subprocess
        root_path = Path(filepath).parent.parent.parent.parent  # Go up to rerun root
        validator = AttrsInitValidator(root_path)

        results = validator.validate_file(Path(filepath))

        errors = []
        for result in results:
            if result.missing_args:
                missing_args = ", ".join(sorted(result.missing_args))
                errors.append(f"__attrs_init__ call on line {result.line_number} missing arguments: {missing_args}")

            if result.extra_args:
                extra_args = ", ".join(sorted(result.extra_args))
                errors.append(f"__attrs_init__ call on line {result.line_number} has extra arguments: {extra_args}")

        return errors

    except Exception as e:
        return [f"Failed to run attrs validation: {e}"]


def _index_to_line_nr(content: str, index: int) -> int:
    """Converts a 0-based index into a 0-based line number."""
    return content[:index].count("\n")


class SourceFile:
    """Wrapper over a source file with some utility functions."""

    def __init__(self, path: str) -> None:
        self.path = os.path.realpath(path)
        self.ext = path.split(".")[-1]
        with open(path, encoding="utf8") as f:
            self.lines = f.readlines()
        self._update_content()

    def _update_content(self) -> None:
        """Sync everything with `self.lines`."""
        self.content = "".join(self.lines)

        # gather lines with a `NOLINT` marker
        # nolints is a dict from code to set of line numbers
        # None key is used for unqualified NOLINT
        self.nolints: dict[str | None, set[int]] = {}
        is_in_nolint_block = False
        for i, line in enumerate(self.lines):
            if "NOLINT" in line:
                # Check for NOLINT: ignore[<code>] format
                if "NOLINT: ignore[" in line:
                    match = re.search(r"NOLINT: ignore\[([^\]]+)\]", line)
                    if match:
                        code = match.group(1)
                        if code not in self.nolints:
                            self.nolints[code] = set()
                        self.nolints[code].add(i)
                    else:
                        # Fallback to unqualified NOLINT if parsing fails
                        if None not in self.nolints:
                            self.nolints[None] = set()
                        self.nolints[None].add(i)
                else:
                    # Unqualified NOLINT
                    if None not in self.nolints:
                        self.nolints[None] = set()
                    self.nolints[None].add(i)

            if "NOLINT_START" in line:
                # Check if this is trying to use the ignore[code] format with NOLINT_START
                if "NOLINT_START: ignore[" in line:
                    raise NotImplementedError(
                        f"NOLINT_START: ignore[<code>] format is not implemented yet. "
                        f"Found at line {i + 1}: {line.strip()}"
                    )
                is_in_nolint_block = True

            if is_in_nolint_block:
                # NOLINT_START/END blocks are always unqualified
                if None not in self.nolints:
                    self.nolints[None] = set()
                self.nolints[None].add(i)
                if "NOLINT_END" in line:
                    is_in_nolint_block = False

    def rewrite(self, new_lines: list[str]) -> None:
        """Rewrite the contents of the file."""
        if new_lines != self.lines:
            self.lines = new_lines
            with open(self.path, "w", encoding="utf8") as f:
                f.writelines(new_lines)
            self._update_content()
            print(f"{self.path} fixed.")

    def should_ignore(self, from_line: int, to_line: int | None = None, code: str | None = None) -> bool:
        """
        Determines if we should ignore a violation.

        NOLINT might be on the same line(s) as the violation or the previous line.

        Args:
            from_line: Starting line number (1-based)
            to_line: Ending line number (1-based), defaults to from_line
            code: Specific error code to check for (e.g., 'py-cls-eq'),
                  or None to check for unqualified NOLINT

        """

        if to_line is None:
            to_line = from_line

        line_range = range(from_line - 1, to_line + 1)

        # Check for specific code if provided
        if code in self.nolints:
            return any(i in self.nolints[code] for i in line_range)
        return False

    def should_ignore_index(self, start_idx: int, end_idx: int | None = None, code: str | None = None) -> bool:
        """Same as `should_ignore` but takes 0-based indices instead of line numbers."""
        return self.should_ignore(
            _index_to_line_nr(self.content, start_idx),
            _index_to_line_nr(self.content, end_idx) if end_idx is not None else None,
            code,
        )

    def error(self, message: str, *, line_nr: int | None = None, index: int | None = None) -> str:
        """Construct an error message. If either `line_nr` or `index` is passed, it's used to indicate a line number."""
        if line_nr is None and index is not None:
            line_nr = _index_to_line_nr(self.content, index)
        if line_nr is None:
            return f"{self.path}:{message}"
        else:
            return f"{self.path}:{line_nr + 1}: {message}"


def lint_file(filepath: str, args: Any) -> int:
    source = SourceFile(filepath)
    num_errors = 0

    error: str | None

    is_in_docstring = False

    prev_line = None
    for line_nr, line in enumerate(source.lines):
        if source.should_ignore(line_nr):
            continue

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

    # Check attrs_init completeness in extension files
    if filepath.endswith("_ext.py"):
        attrs_errors = lint_attrs_init_completeness(filepath)
        for error in attrs_errors:
            print(source.error(error))
        num_errors += len(attrs_errors)

    if filepath.endswith((".rs", ".fbs")):
        errors, lines_out = lint_vertical_spacing(source.lines)
        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

        # Check for pyclass requirements (eq and module) in rerun_py Rust files
        if filepath.startswith("./rerun_py/") and filepath.endswith(".rs"):
            pyclass_errors, error_lines, error_codes = lint_pyclass_requirements(source.lines)
            valid_errors = 0
            for error, line_number, error_code in zip(pyclass_errors, error_lines, error_codes, strict=True):
                if not source.should_ignore(line_number, code=error_code):
                    print(
                        source.error(error)
                        + f"\n\tUnqualified NOLINT not allowed for pyclass lints. Use `NOLINT: ignore[{error_code}]` instead."
                    )
                    valid_errors += 1
            num_errors += valid_errors

            # Check for pymethods requirements (__str__ method) in rerun_py Rust files
            pymethods_errors, pymethods_error_lines, pymethods_error_codes = lint_pymethods_requirements(source.lines)
            valid_pymethods_errors = 0
            for error, line_number, error_code in zip(
                pymethods_errors, pymethods_error_lines, pymethods_error_codes, strict=True
            ):
                if not source.should_ignore(line_number, code=error_code):
                    print(
                        source.error(error)
                        + f"\n\tUnqualified NOLINT not allowed for pymethods lints. Use `NOLINT: ignore[{error_code}]` instead."
                    )
                    valid_pymethods_errors += 1
            num_errors += valid_pymethods_errors

        if args.fix:
            source.rewrite(lines_out)

    if filepath.endswith(".md"):
        errors, lines_out = lint_markdown(filepath, source)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

        if args.fix:
            source.rewrite(lines_out)
        elif 0 < num_errors:
            print(f"Run with --fix to automatically fix {num_errors} errors.")

    if not filepath.startswith("./examples/rust") and filepath != "./Cargo.toml" and filepath.endswith("Cargo.toml"):
        error = lint_workspace_lints(source.content)

        if error is not None:
            print(source.error(error))
            num_errors += 1

    # Markdown-specific lints
    if filepath.endswith(".md"):
        errors = lint_frontmatter(filepath, source.content)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

    return num_errors


def lint_crate_docs() -> int:
    """Make sure ARCHITECTURE.md talks about every single crate we have."""

    crates_dir = Path("crates")
    architecture_md_file = Path("ARCHITECTURE.md")

    architecture_md = architecture_md_file.read_text("utf-8")

    # extract all crate names ("re_…") from ARCHITECTURE.md to ensure they actually exist
    listed_crates: dict[str, int] = {}
    for i, line in enumerate(architecture_md.split("\n"), start=1):
        for crate_name in re.findall(r"\bre_\w+", line):
            if crate_name not in listed_crates:
                listed_crates[crate_name] = i

    error_count = 0
    for cargo_toml in crates_dir.glob("**/Cargo.toml"):
        crate = cargo_toml.parent
        crate_name = crate.name

        listed_crates.pop(crate_name, None)

        if not re.search(r"\b" + crate_name + r"\b", architecture_md):
            print(f"{architecture_md_file}: missing documentation for crate {crate.name}")
            error_count += 1

    for crate_name, line_nr in sorted(listed_crates.items(), key=lambda x: x[1]):
        print(f"{architecture_md_file}:{line_nr}: crate name {crate_name} does not exist")
        error_count += 1

    return error_count


def main() -> None:
    # Make sure we are bug free before we run:
    test_split_words()
    test_lint_line()
    test_lint_vertical_spacing()
    test_lint_pyclass_requirements()
    test_lint_pymethods_requirements()
    test_is_emoji()

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
    parser.add_argument(
        "--extra",
        dest="extra",
        action="store_true",
        help="Run some extra checks.",
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
        "./.nox",
        "./.pytest_cache",
        "./CODE_STYLE.md",
        "./crates/build/re_types_builder/src/reflection.rs",  # auto-generated
        "./crates/store/re_protos/proto/schema_snapshot.yaml",  # auto-generated
        "./crates/store/re_protos/src/v0",  # auto-generated
        "./crates/store/re_protos/src/v1alpha1",  # auto-generated
        "./crates/viewer/re_web_viewer_server/web_viewer/re_viewer.js",  # auto-generated by wasm_bindgen
        "./docs/content/concepts/app-model.md",  # this really needs custom letter casing
        "./docs/content/reference/cli.md",  # auto-generated
        "./docs/snippets/all/tutorials/custom-application-id.cpp",  # nuh-uh, I don't want rerun_example_ here
        "./docs/snippets/all/tutorials/custom-application-id.py",  # nuh-uh, I don't want rerun_example_ here
        "./docs/snippets/all/tutorials/custom-application-id.rs",  # nuh-uh, I don't want rerun_example_ here
        "./examples/assets",
        "./examples/python/detect_and_track_objects/cache/version.txt",
        "./examples/python/objectron/objectron/proto/",  # auto-generated
        "./examples/rust/objectron/src/objectron.rs",  # auto-generated
        "./rerun_cpp/docs/doxygen-awesome/",  # copied from an external repository
        "./rerun_cpp/docs/html",
        "./rerun_cpp/src/rerun/c/arrow_c_data_interface.h",  # Not our code
        "./rerun_cpp/src/rerun/third_party/cxxopts.hpp",  # vendored
        "./rerun_js/docs/",  # auto-generated
        "./rerun_js/node_modules",
        "./rerun_js/web-viewer-react/node_modules",
        "./rerun_js/web-viewer/index.js",
        "./rerun_js/web-viewer/inlined.js",
        "./rerun_js/web-viewer/node_modules",
        "./rerun_js/web-viewer/re_viewer_bg.js",  # auto-generated by wasm_bindgen
        "./rerun_js/web-viewer/re_viewer.js",
        "./rerun_notebook/node_modules",
        "./rerun_notebook/src/rerun_notebook/static",
        "./rerun_py/.pytest_cache/",
        "./rerun_py/site/",  # is in `.gitignore` which this script doesn't fully respect
        "./run_wasm/README.md",  # Has a "2d" lowercase example in a code snippet
        "./scripts/lint.py",  # we contain all the patterns we are linting against
        "./scripts/zombie_todos.py",
        "./tests/assets/lerobot/apple_storage/README.md",  # not ours
        "./tests/python/gil_stress/main.py",
        "./tests/python/release_checklist/main.py",
    )

    should_ignore = parse_gitignore(".gitignore")  # TODO(#6730): parse all .gitignore files, not just top-level

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
        repo = git.Repo(".", search_parent_directories=True)
        tracked_files = [item[1].path for item in repo.index.iter_blobs()]
        for filepath in tracked_files:
            # TODO do this with pathlib for general sep types
            filepath = "./" + filepath
            extension = filepath.split(".")[-1]
            if extension in extensions:
                if filepath.startswith(exclude_paths):
                    continue
                num_errors += lint_file(filepath, args)

        # Since no files have been specified, we also run the global lints.
        num_errors += lint_crate_docs()

    if num_errors == 0:
        print(f"{sys.argv[0]} finished without error")
        sys.exit(0)
    else:
        print(f"{sys.argv[0]} found {num_errors} errors.")
        sys.exit(1)


if __name__ == "__main__":
    main()
