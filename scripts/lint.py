#!/usr/bin/env python3

"""
Runs custom linting on our code.

Adding "NOLINT" to any line makes the linter ignore that line. Adding a pair of "NOLINT_START" and "NOLINT_END" makes
the linter ignore these lines, as well as all lines in between.

If you are linting Rust code, consider using `relint` instead,
which has access to a proper AST of the source code.

A NOLINT that suppresses nothing is an error, so that NOLINTs don't outlive the code they were written for.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from concurrent.futures import ProcessPoolExecutor
from contextlib import redirect_stdout
from functools import cache
from io import StringIO
from pathlib import Path
from typing import Any

import git
import yaml
from ci.frontmatter import load_frontmatter
from gitignore_parser import parse_gitignore

# -----------------------------------------------------------------------------

# Path prefix from git root to the rerun directory.
# "./rerun/" in the monorepo (reality), "./" in the standalone rerun repo.
# Set in main().
rerun_prefix = "./"

wasm_caps = re.compile(r"\bWASM\b")
else_return = re.compile(r"else\s*{\s*return;?\s*};")
# Looks for: \"{foo}\" (manual quotes), including \"{foo:?}\" (excess quotes).
explicit_quotes = re.compile(r'[^(]\\"\{[^}]*\}\\"')
# Three dots between two words or `{placeholders}` are a range (e.g. `main...HEAD`, `{sha}...{tag}`), not an ellipsis.
ellipsis = re.compile(r"[^.](?<![\w}])\.\.\.([^\-.0-9a-zA-Z]|$)|[^.]\.\.\.([^\-.0-9a-zA-Z{]|$)")
ellipsis_expression = re.compile(r"[\[\]\(\)<>\{\}]?.*\.\.\..*[\[\]\(\)<>\{\}]")
# A `///` comment, either opening a line or trailing a declaration. `////` separator lines are fine.
triple_slash_comment = re.compile(r"(?:^|\s)///(?!/)")

double_space = re.compile(r"[.a-zA-Z]  [a-zA-Z]")
double_the = re.compile(r"\bthe the\b")
double_word = re.compile(r" ([a-z]+) \1[ \.]")
url_pattern = re.compile(r'https?://[^ )"]+')
markdown_link_target = re.compile(r"\]\([^)]*\)")
lowercase_2d = re.compile(r"\b2d\b")
lowercase_3d = re.compile(r"\b3d\b")
unspaced_em_dash = re.compile(r"[\w\)\*]—[\w\(\*]")
sentence_en_dash = re.compile(r"[A-Za-z\"'\)]\s–\s[A-Za-z]")
todo_owner = re.compile(r"TODO\(([^)]*)\)")
malformed_todo = re.compile(r'TODO([^_"(]|$)')
debug_formatted_ui_label = re.compile(r"ui\.\w+\([^)]*\n?[^)]*:\?")
quoted_string = re.compile(r'"([^"]*)"')
deprecated_rerun_cloud = re.compile(r"\bRerun\s+Cloud\b", re.IGNORECASE)
deprecated_rerun_base = re.compile(r"\bRerun\s+Base\b", re.IGNORECASE)
deprecated_rerun_data_platform = re.compile(r"\bRerun\s+Data\s+Platform\b", re.IGNORECASE)
deprecated_data_platform = re.compile(r"\bData\s+Platform\b", re.IGNORECASE)
deprecated_cloud_instance = re.compile(r"\byour\s+cloud\s+instances?\b", re.IGNORECASE)
deprecated_dataplatform = re.compile(r"(?<![/\[,])dataplatform(?![/\],])", re.IGNORECASE)
rerun_hub = re.compile(r"\bRerun\s+Hub\b", re.IGNORECASE)
recording_stream_app_id = re.compile(r'(RecordingStreamBuilder::new|\.init|RecordingStream)\(\s*"([^"]*)')
script_setup_app_id = re.compile(r'(rr.script_setup)\(\s*args,\s*"(\w*)')

# reStructuredText syntax that we don't want in Python docstrings.
# Our Python API docs use MkDocs + mkdocstrings (not Sphinx), so rST isn't rendered.
# See `CLAUDE.md` → "Python docstring formatting".
rst_role = re.compile(r":(class|meth|func|mod|attr|exc|data|const|obj|ref):`")
rst_directive = re.compile(
    r"\.\.\s+(warning|note|deprecated|code-block|seealso|versionadded|versionchanged|admonition|caution|danger|hint|important|tip|attention|toctree|automodule|autoclass|autofunction)::"
)
# rST inline literal — ``code``. Markdown uses single backticks.
# Avoid matching triple-backtick code fences.
rst_double_backtick = re.compile(r"(?<!`)``[^`\n]+``(?!`)")

Frontmatter = dict[str, Any]


def get_rerun_root() -> str:
    # Search upward for .RERUN_ROOT sentinel file
    # TODO(RR-3355): Use a shared utility for this
    current = Path(__file__).resolve().parent
    while current != current.parent:
        # Look for sentinel file
        if (current / ".RERUN_ROOT").exists():
            return str(current)
        # Break if we reach a git root
        if (current / ".git").exists():
            break
        current = current.parent
    raise FileNotFoundError(f"Could not find .RERUN_ROOT sentinel file in any parent directory under {current}")


def is_valid_todo_part(part: str) -> bool:
    part = part.strip()

    if re.match(r"^[\w/-]*#\d+$", part):
        return True  # org/repo#42 or #42

    if part.lower() in ("agent", "claude", "codex", "llm"):
        return False  # coding agent, not a person

    if re.match(r"^[a-z][a-z0-9_]+$", part):
        return True  # user-name

    return bool(re.match(r"^RR-\d+$", part))  # linear issue


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

    # A capitalized word followed by `<`, `::`, `(`, or `{` is Rust code, not prose.
    if m := re.search(r"[^.] ([A-Z]\w+)\b(?!<|::|\(|\{)", s):
        word = m.group(1)
        if word in bad_titles:
            return f"Do not use title casing ({word}). See https://github.com/rerun-io/rerun/blob/main/DESIGN.md"

    return None


def lint_url(url: str) -> str | None:
    ALLOW_LIST_URLS = {
        "https://github.com/lycheeverse/lychee/blob/master/lychee.example.toml",
        "https://github.com/rerun-io/documentation/blob/main/src/utils/tokens.ts",
        "https://github.com/rerun-io/landing/blob/main/src/lib/lang.ts",  # if this file moves we should check the linked code.
        "https://github.com/rerun-io/platform-operator/blob/main/README-tech-overview.md",
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
            elif url.startswith("https://github.com/anthropics/claude-code-action"):
                pass
            else:
                return f"Do not link directly to a file on '{branch}' - it may disappear! Use a commit hash or tag instead. Url: {url}"

    return None


docs_url = re.compile(r"rerun\.io/docs/([\w\-/]+)")


@cache
def redirected_doc_paths() -> dict[str, str]:
    """Old docs paths from `_redirects.yaml`, mapped to where they redirect."""
    with open(f"{rerun_prefix}docs/content/_redirects.yaml", encoding="utf-8") as f:
        redirects: dict[str, str] = yaml.safe_load(f) or {}
    return redirects


def lint_docs_redirect_links(filepath: str, content: str, redirects: dict[str, str]) -> list[tuple[int, str]]:
    """Links to an old docs path that only works through a redirect, as `(index, error)`."""
    is_changelog = "CHANGELOG.md" in filepath or "docs/content/changelog/" in filepath
    if is_changelog:
        return []  # History links to the paths of its time.

    errors = []
    for match in docs_url.finditer(content):
        path = match.group(1).strip("/")
        if new_path := redirects.get(path):
            errors.append((match.start(), f"`docs/{path}` is a redirect: link to `docs/{new_path}` instead"))
    return errors


absolute_docs_link = re.compile(r"https?://(?:www\.)?rerun\.io/docs/[^\s)\"'>]*")
python_sdk_source_link = re.compile(
    r"https://github\.com/rerun-io/rerun/(?:blob|tree)/[^\s)\"'>]*/rerun_py/rerun_sdk/[^\s)\"'>]*"
)
version_anchor = re.compile(
    r"\b(?:[Ss]ince|[Aa]s of|[Ss]tarting (?:with|from)) (?:Rerun )?`?v?0\.\d+|\b[Ii]n Rerun `?v?0\.\d+"
)


def docs_page_frontmatter(content: str) -> dict[str, Any]:
    """The YAML frontmatter of a `docs/content/` page, or `{}` if it has none."""
    if not content.startswith("---\n"):
        return {}
    end = content.find("\n---", 4)
    if end == -1:
        return {}
    try:
        frontmatter = yaml.safe_load(content[4:end])
    except yaml.YAMLError:
        return {}
    return frontmatter if isinstance(frontmatter, dict) else {}


@cache
def documented_frontmatter_keys() -> frozenset[str]:
    """The frontmatter keys listed in the first YAML block of the "Frontmatter" section of `docs/README.md`."""
    readme = (Path(__file__).resolve().parent.parent / "docs" / "README.md").read_text(encoding="utf-8")
    section = readme[readme.index("### Frontmatter") :]
    block_start = section.index("```yaml\n") + len("```yaml\n")
    block = section[block_start : section.index("```", block_start)]
    return frozenset(re.findall(r"^(\w+):", block, re.MULTILINE))


def mask_html_comments(line: str, in_comment: bool) -> tuple[str, bool]:
    """
    `line` with everything inside `<!-- … -->` comments replaced by spaces, and whether a comment is still open at its end.

    The masked line has the same length as `line`, so match positions in it are positions in `line`.
    """
    masked = ""
    pos = 0
    while pos < len(line):
        if in_comment:
            end = line.find("-->", pos)
            if end == -1:
                return masked + " " * (len(line) - pos), True
            masked += " " * (end + 3 - pos)
            pos = end + 3
            in_comment = False
        else:
            start = line.find("<!--", pos)
            if start == -1:
                return masked + line[pos:], False
            masked += line[pos:start] + "    "
            pos = start + 4
            in_comment = True
    return masked, in_comment


def lint_docs_page(filepath: str, content: str) -> list[tuple[int, str]]:
    """Rules for the hand-written pages in `docs/content/`, as `(index, error)`."""
    if "docs/content/" not in filepath or not filepath.endswith(".md"):
        return []
    if "docs/content/changelog/changeset-" in filepath or "docs/content/reference/migration/" in filepath:
        return []  # History: it keeps the links and wording of its time.
    if "docs/content/reference/types/" in filepath:
        return []  # Generated from the type definitions, whose docs also become the SDKs' docstrings.

    errors = []

    frontmatter = docs_page_frontmatter(content)
    if frontmatter.get("hidden") is True and "redirect" in frontmatter:
        errors.append((0, "A hidden redirect page: move the redirect to `docs/content/_redirects.yaml` instead"))

    is_upcoming_changelog = "docs/content/changelog/upcoming/" in filepath

    # Changelog entries have their own keys, documented in `changelog/upcoming/_template.md`.
    if not is_upcoming_changelog:
        for key in sorted(frontmatter.keys() - documented_frontmatter_keys()):
            errors.append((0, f"Undocumented frontmatter key `{key}`: add it to the list in `docs/README.md`"))

    in_code_block = False
    in_html_comment = False
    line_start = 0
    for line in content.splitlines(keepends=True):
        if line.lstrip().startswith("```") and not in_html_comment:
            if not in_code_block and line.strip() == "```py":
                errors.append((line_start, "Use ```python, not ```py: the website does not highlight `py`"))
            in_code_block = not in_code_block
        elif not in_code_block:
            # Only outside code blocks: a `<!--` in example code does not open a comment.
            visible, in_html_comment = mask_html_comments(line, in_html_comment)
            for match in absolute_docs_link.finditer(visible):
                errors.append((
                    line_start + match.start(),
                    f"Link to docs pages with a relative `.md` path, not `{match.group()}`: relative links are checked against this repo, and work on GitHub",
                ))
            for match in python_sdk_source_link.finditer(visible):
                errors.append((
                    line_start + match.start(),
                    "Link to the Python reference docs on `ref.rerun.io`, not to the SDK source on GitHub",
                ))
            # A `breaking` changelog entry is its change's migration guide, which may name versions.
            if not is_upcoming_changelog:
                for match in version_anchor.finditer(visible):
                    errors.append((
                        line_start + match.start(),
                        f"Describe the current behavior instead of when it changed (`{match.group()}`): version history belongs in the changelog and migration guides",
                    ))
            # Changelog entries may hold `TODO(name)` placeholders: the release is blocked until they are resolved.
            if "TODO" in visible and not is_upcoming_changelog:
                errors.append((
                    line_start + visible.find("TODO"),
                    "Readers see this TODO: state the current limitation instead, or move the TODO into an `<!-- … -->` comment",
                ))
        line_start += len(line)

    return errors


def lint_example_app_ids(content: str) -> list[tuple[int, str]]:
    """Application ids that don't start with `rerun_example_`, as `(index, error)`."""
    errors = []
    for match in [*recording_stream_app_id.finditer(content), *script_setup_app_id.finditer(content)]:
        app_id = match.group(2)
        if not app_id.startswith("rerun_example_") and app_id != "<your_app_name>":
            errors.append((
                match.start(),
                f"All examples should have an app_id starting with 'rerun_example_'. Found '{app_id}'",
            ))
    return errors


def lint_line(
    line: str,
    prev_line: str | None,
    file_extension: str = "rs",
    is_in_docstring: bool = False,
    is_in_oss_rerun_repo: bool = True,
) -> str | None:
    if line == "":
        return None

    if line[-1].isspace():
        return "Trailing whitespace"

    if has_nolint(line):
        return None  # NOLINT ignores the linter

    if file_extension not in ("py", "txt", "yaml", "yml"):
        if "Github" in line:
            return "It's 'GitHub', not 'Github'"

        if " github " in line:
            return "It's 'GitHub', not 'github'"

    if double_space.search(line):
        if r"\n  " not in line:  # Allow `\n  `, which happens e.g. when markdown is embedded in a string
            return "Found double space"

    if double_the.search(line.lower()):
        return "Found 'the the'"

    if m := double_word.search(line):
        return f"Found double word: '{m.group(0)}'"

    if m := url_pattern.search(line):
        found_url = m.group(0)
        if err := lint_url(found_url):
            return err

    if file_extension != "" and is_in_oss_rerun_repo:
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
                and "from ..." not in line
                and line.strip() != "..."
                and "&..." not in line
            ):
                return "Use … instead of ... (on Mac it's option+;)"

    if "http" not in line:
        # Strip markdown link/image destinations to avoid false positives on file paths (e.g. `/3d-camera.png`)
        line_without_link_targets = markdown_link_target.sub("]()", line)
        if lowercase_2d.search(line_without_link_targets):
            return "we prefer '2D' over '2d'"
        if lowercase_3d.search(line_without_link_targets):
            return "we prefer '3D' over '3d'"

    # Em dash should be spaced (` — `, not `word—word`). See DESIGN.md.
    # The UI placeholder literal `"—"` (em dash inside quotes) is naturally exempt
    # since the regex requires word/paren/asterisk characters on both sides.
    if unspaced_em_dash.search(line):
        return "Use a spaced em dash (' — '), not 'word—word'. See DESIGN.md."

    # En dash (`–`) is for numeric ranges only; as a sentence dash, use an em dash (` — `).
    # Detection: en dash with spaces, flanked by letters (digit-flanked allows `100 – 200`).
    if sentence_en_dash.search(line):
        return "Use an em dash (' — '), not an en dash, as a sentence dash. See DESIGN.md."

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

    if m := todo_owner.search(line):
        parts = m.group(1).split(",")
        if len(parts) == 0 or not all(is_valid_todo_part(p) for p in parts):
            return "TODOs should be formatted as either TODO(name), TODO(#42) or TODO(org/repo#42)"

    if malformed_todo.search(line):
        return "TODO:s should be written as `TODO(yourname): what to do`"

    if "from attr import dataclass" in line:
        return "Avoid 'from attr import dataclass'; prefer 'from dataclasses import dataclass'"

    if m := else_return.search(line):
        match = m.group(0)
        if match != "else { return; };":
            # Because cargo fmt doesn't handle let-else
            return f"Use 'else {{ return; }};' instead of '{match}'"

    if wasm_caps.search(line):
        return "WASM should be written 'Wasm'"

    if "nb_" in line:
        return "Don't use nb_things - use num_things or thing_count instead"

    if explicit_quotes.search(line):
        return (
            "Prefer using {:?} over explicit quotes - it will also escape newlines etc. "
            "See: https://github.com/rerun-io/rerun/blob/main/CODE_STYLE.md#misc"
        )

    if m := quoted_string.search(line):
        if err := check_string(m.group(1)):
            return err

    if "rec_stream" in line or "rr_stream" in line:
        return "Instantiated RecordingStreams should be named `rec`"

    # rST syntax is not rendered by MkDocs/mkdocstrings (our Python API docs).
    # Python docstrings and Rust `///` doc comments (pyo3 exposes them as Python docstrings).
    is_in_rust_doc_comment = file_extension == "rs" and line.lstrip().startswith("///")
    if is_in_docstring or is_in_rust_doc_comment:
        if m := rst_role.search(line):
            role = m.group(1)
            return (
                f"Found rST role `:{role}:` in docstring — use markdown/mkdocstrings cross-references "
                "like `[`Name`][]` or `[`Name`][module.Name]` instead "
                "(our Python API docs are built with MkDocs, not Sphinx)"
            )
        if m := rst_directive.search(line):
            directive = m.group(1)
            if directive == "deprecated":
                return (
                    "Found rST `.. deprecated::` directive — use the `@deprecated` decorator instead "
                    "(mkdocstrings renders it automatically)"
                )
            if directive == "code-block":
                return "Found rST `.. code-block::` directive — use markdown fenced code blocks (```lang) instead"
            return (
                f"Found rST `.. {directive}::` directive — use a MkDocs admonition "
                f"(`!!! {directive}` with an indented body) instead"
            )
        if rst_double_backtick.search(line):
            return (
                "Found rST double-backtick literal ``…`` — use a single backtick `…` in markdown "
                "(our Python API docs are built with MkDocs, not Sphinx)"
            )

    # Deprecated brand names. Replacement is context-dependent:
    #   - 'Rerun Hub'      → commercial managed offering
    #   - 'catalog server' → generic OSS or managed
    #   - or rephrase to avoid naming the product
    # Matched case-insensitively so that lowercase variants (e.g. 'rerun cloud') are also caught.
    deprecated_msg = "is a deprecated name. Use 'Rerun Hub' (commercial), 'catalog server' (generic), or rephrase."
    if deprecated_rerun_cloud.search(line):
        return f"'Rerun Cloud' {deprecated_msg}"
    if deprecated_rerun_base.search(line):
        return f"'Rerun Base' {deprecated_msg}"
    if deprecated_rerun_data_platform.search(line):
        return f"'Rerun Data Platform' {deprecated_msg}"
    if deprecated_data_platform.search(line):
        return f"'Data Platform' {deprecated_msg}"
    # Only in the OSS rerun repo: elsewhere "cloud instance" is a generic infrastructure term (an EC2 instance, …).
    if is_in_oss_rerun_repo and deprecated_cloud_instance.search(line):
        return f"'your cloud instance' {deprecated_msg}"
    # The bare word `dataplatform` is only checked in the OSS rerun repo:
    # in the monorepo it is the literal name of the workspace directory, docker image,
    # pixi tasks, env vars, etc., so it is legitimate there.
    # Skip URL paths (`/dataplatform/`) and python package extras specifiers
    # (`rerun-sdk[dataloader,dataplatform]`) — those name a directory or an extra, not prose.
    # The `dataplatform` extra was removed in 0.37, but changelogs and migration guides still quote it.
    if is_in_oss_rerun_repo and deprecated_dataplatform.search(line):
        return f"'dataplatform' {deprecated_msg}"

    # Enforce 'Rerun Hub' capitalization: flag any case variant that isn't exactly 'Rerun Hub'.
    for m in rerun_hub.finditer(line):
        if m.group(0) != "Rerun Hub":
            return "'Rerun Hub' must be properly capitalized."

    return None


def test_lint_line() -> None:
    assert lint_line("hello world", None) is None

    should_pass = [
        'lint_rule("fn f() -> Result<(), String> {}")',
        'lint("fn f() { Result::<(), E>::Ok(()) }")',
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
        "WASM_FOO env var",
        "Wasm",
        "num_instances",
        "instances_count",
        "let Some(foo) = bar else { return; };",
        "{foo:?}",
        'ui.label("This is fine. Correct casing.")',
        'ui.label(format!("Value: {value}"));',
        "rec",
        "The theme is great",
        "template <typename... Args>",
        'f"repos/{OWNER}/rerun/compare/{sha}...{tag}"',
        '_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])',
        # URL paths and python package extras still reference the feature name.
        "Visit /dataplatform/docs for more info",
        "The https://example.com/dataplatform/api endpoint",
        'dependencies = ["rerun-sdk[dataloader,dataplatform]"]',
        'override-dependencies = ["rerun-sdk[dataplatform]"]',
        'extras = ["dataplatform,extra"]',
        # New approved names.
        "Connect to Rerun Hub for hosted catalogs.",
        "Spin up a catalog server locally.",
        "We use the catalog server in production.",
        # Spaced em dash is the convention.
        "Use a spaced em dash (` — `) for parenthetical breaks.",
        "foo — bar — baz",
        # En dash is fine in numeric/character ranges (no spaces, or digit-flanked).
        "Range: 2020–2025",
        "pp. 10–15",
        "100 KB–10 MB",
        "Chunks 100 – 200",  # digit on right side — allowed range with spaces
        "A–Z and a–z and 0–9",
        # Em dash as a UI placeholder literal in a string (not prose).
        '"—".to_owned()',
        'return sha[:8] if sha else "—"',
        # Mathematical/UI display with en dash, no spaces.
        'ui.button("–∞")',
    ]

    should_error = [
        "this is a 2d view",
        "FIXME",
        "HACK",
        "TODO",
        "# TODO make",
        "TODO:",
        "TODO(42)",
        "TODO(https://github.com/rerun-io/rerun/issues/42)",
        "TODO(bob/alice)",
        "TODO(bob|alice)",
        "TODO(agent)",  # TODOs left for a coding agent
        "TODO(Claude)",  # TODOs left for a coding agent
        "TODO(codex)",  # TODOs left for a coding agent
        "TODO(llm)",  # TODOs left for a coding agent
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
        "The the problem with double words",
        "More than meets the eye...",
        'print(f"Loading {name}...")',
        're_log::trace!("Performing migrations...");',
        'rr.log("/", rr.TextLog("Logging things..."))',
        'logging.info("Detection finished...")',
        "I accidentally wrote the same same word twice",
        # Deprecated brand names — must use 'Rerun Hub' or 'catalog server' instead.
        # Matched case-insensitively, so lowercase variants must also error.
        "The dataplatform is powerful",
        "Using dataplatform for analytics",
        "Using DATAPLATFORM in caps",
        "I love the data platform",
        "The Rerun data platform is great",
        "We use the Rerun Data Platform.",
        "We use the RERUN DATA PLATFORM.",
        "Connect via Rerun Cloud today.",
        "Connect via rerun cloud today.",
        "Connect via RERUN CLOUD today.",
        "The Data Platform stores recordings.",
        "The data platform stores recordings.",
        "Rerun Base is the new commercial offering.",
        "rerun base is the new commercial offering.",
        "In practice you'll connect to your cloud instance.",
        # Wrong 'Rerun Hub' capitalization.
        "Connect to Rerun hub today.",
        "Use rerun Hub for catalogs.",
        "Use rerun hub for catalogs.",
        "USE RERUN HUB FOR CATALOGS.",
        # Unspaced em dash (should be spaced).
        "the layout—are computed",
        "components—the viewer no longer",
        "*data blueprints*—the entity",
        "(SN)—and the storage node",
        # En dash used as a sentence dash (should be em dash).
        "Foo – the description",
        "[Python](./install-rerun/python.md) – the Python SDK",
        "done – next step",
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

    assert debug_formatted_ui_label.search('ui.label(format!("Value: {value:?}"));')
    assert debug_formatted_ui_label.search('ui.label(format!(\n    "Value: {value:?}",\n));')
    assert debug_formatted_ui_label.search('ui.error_label(format!("Value: {value:?}"));')
    assert not debug_formatted_ui_label.search('ui.label(format!("Value: {value}"));')
    assert not debug_formatted_ui_label.search('ui.error_label(format!("Value: {value}"));')

    # Deprecated brand names are checked everywhere in the monorepo…
    assert lint_line("Connect via Rerun Cloud today.", None, is_in_oss_rerun_repo=False) is not None
    assert lint_line("We use the Rerun Data Platform.", None, is_in_oss_rerun_repo=False) is not None
    # …except the bare word 'dataplatform', which legitimately names the workspace directory there.
    assert lint_line("The dataplatform is powerful", None, is_in_oss_rerun_repo=True) is not None
    assert lint_line("The dataplatform is powerful", None, is_in_oss_rerun_repo=False) is None
    assert lint_line("Launch an EC2 cloud instance with a GPU.", None) is None
    assert lint_line("Connect to your cloud instance.", None, is_in_oss_rerun_repo=False) is None

    # rST (reStructuredText) is not rendered by MkDocs/mkdocstrings.
    # Flagged inside Python docstrings and Rust `///` doc comments only.
    rst_should_fail_in_docstring = [
        "A :class:`Foo` object.",
        "See :meth:`Foo.bar` for details.",
        "Use :func:`rerun.init` to start.",
        "Reference :attr:`Foo.x`.",
        ".. warning::",
        "    .. warning::",
        ".. note:: This is important",
        ".. deprecated:: 0.1",
        ".. code-block:: python",
        ".. seealso:: related",
        "Handles ``list<double>`` and ``list<list<double>>``.",
        "Returns ``None`` when empty.",
    ]
    for line in rst_should_fail_in_docstring:
        assert lint_line(line, None, "py", is_in_docstring=True) is not None, (
            f'expected "{line}" to fail inside a Python docstring'
        )
        # Same lines should not fire outside docstrings (e.g. in regular code/comments).
        assert lint_line(line, None, "py", is_in_docstring=False) is None, (
            f'expected "{line}" to pass outside a Python docstring, but it was flagged'
        )
        # Rust `///` doc comments (exposed as Python docstrings via pyo3) are checked too.
        assert lint_line(f"/// {line}", None, "rs") is not None, f'expected "/// {line}" to fail in a Rust doc comment'
        # Regular `//` comments are not checked.
        assert lint_line(f"// {line}", None, "rs") is None, f'expected "// {line}" to pass in a regular Rust comment'

    rst_should_pass_in_docstring = [
        "Use [`Foo`][] instead.",
        "Reference [`Foo.bar`][rerun.Foo.bar].",
        "!!! warning",
        "    !!! warning",
        "!!! note",
        "A regular sentence with no rST.",
        "```python",
        "Handles `list<double>` and `list<list<double>>`.",
        # Parameter section headers (numpy style) — these look superficially similar but aren't rST directives.
        "Parameters",
        "----------",
    ]
    for line in rst_should_pass_in_docstring:
        err = lint_line(line, None, "py", is_in_docstring=True)
        assert err is None, f'expected "{line}" to pass inside a Python docstring, but got: "{err}"'


# -----------------------------------------------------------------------------


workspace_lints = re.compile(r"\[lints\]\nworkspace\s*=\s*true")


def lint_workspace_lints(cargo_file_content: str) -> str | None:
    """Checks that a non-example cargo file has a lints section that sets workspace to true."""

    if workspace_lints.search(cargo_file_content):
        return None
    else:
        return "Non-example cargo files should have a [lints] section with workspace = true"


def lint_workspace_dependency_headings(cargo_file_content: str) -> str | None:
    """Checks that each `# crates/…:` heading in `[workspace.dependencies]` matches the paths under it."""

    heading = None
    for line in cargo_file_content.splitlines():
        match = re.match(r"# (crates/[a-z_]+):", line)
        if match:
            heading = match.group(1)
            continue
        match = re.match(r'([a-z_0-9-]+) = \{ path = "(crates/[a-z_]+)/', line)
        if match and heading is not None and match.group(2) != heading:
            return (
                f"`{match.group(1)}` is listed under `# {heading}:` but lives in `{match.group(2)}`. "
                "Move it under the right heading."
            )
    return None


# -----------------------------------------------------------------------------

force_capitalized = [
    "2D",
    "3D",
    "Apache",
    "API",
    "APIs",
    "April",
    "Bayer",
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
    "Windows",
    # "Arrow",   # Would be nice to capitalize in the right context, but it's a too common word.
    # "Windows", # Consider "multiple plot windows"
]

allow_capitalized = [
    "Viewer",
    # Referring to the Rerun Viewer as just "the Viewer" is fine, but not all mentions of "viewer" are capitalized.
    "Arrow",
    # Referring to the Apache Arrow project as just "Arrow" is fine, but not all mentions of "arrow" are capitalized.
    "Hub",
    # Referring to Rerun Hub as just "Hub" is fine, but "hub" as a common noun isn't capitalized.
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


def test_lint_docs_redirect_links() -> None:
    redirects = {"old/page": "new/page"}

    def errors(filepath: str, content: str) -> list[str]:
        return [error for _, error in lint_docs_redirect_links(filepath, content, redirects)]

    expected = ["`docs/old/page` is a redirect: link to `docs/new/page` instead"]
    assert errors("README.md", "https://rerun.io/docs/old/page") == expected
    assert errors("README.md", "https://rerun.io/docs/old/page/#section") == expected
    assert errors("README.md", "https://rerun.io/docs/new/page") == []
    assert errors("README.md", "https://rerun.io/docs/old/page-two") == []
    assert errors("CHANGELOG.md", "https://rerun.io/docs/old/page") == []
    assert errors("docs/content/changelog/changeset-0-42.md", "https://rerun.io/docs/old/page") == []


def test_lint_docs_page() -> None:
    def num_errors(content: str, filepath: str = "docs/content/viewer/page.md") -> int:
        return len(lint_docs_page(filepath, content))

    # Frontmatter keys must be listed in `docs/README.md`.
    assert num_errors("---\ntitle: A\norder: 1\nindex: true\n---\n") == 0
    assert num_errors("---\ntitle: A\nsidebar: false\n---\n") == 1
    assert num_errors("---\ntitle: A\ntype: feature\n---\n", filepath="docs/content/changelog/upcoming/x.md") == 0

    # Absolute links to docs pages.
    assert num_errors("See [views](https://rerun.io/docs/viewer/views).\n") == 1
    assert num_errors("See [views](../viewer/views.md).\n") == 0
    assert num_errors("```python\nprint('https://rerun.io/docs/viewer')\n```\n") == 0
    assert num_errors("See https://rerun.io/docs/viewer.\n", filepath="README.md") == 0
    assert num_errors("See https://rerun.io/docs/viewer.\n", filepath="docs/content/changelog/changeset-0-42.md") == 0
    assert num_errors("See https://rerun.io/docs/viewer.\n", filepath="docs/content/changelog/upcoming/x.md") == 1

    # Links to the Python SDK source.
    url = "https://github.com/rerun-io/rerun/blob/main/rerun_py/rerun_sdk/rerun/urdf.py"
    assert num_errors(f"The [`urdf`]({url}) module.\n") == 1
    assert num_errors("The [example](https://github.com/rerun-io/rerun/tree/main/examples/python/dna).\n") == 0

    # Version anchors.
    assert num_errors("Since Rerun 0.28, transforms are logged per frame.\n") == 1
    assert num_errors("In Rerun `v0.30` we added multiple visualizers.\n") == 1
    assert num_errors("As of 0.31, this is the default.\n") == 1
    assert num_errors("Wait in 0.5 second steps.\n") == 0
    assert num_errors("Since Rerun 0.28, …\n", filepath="docs/content/reference/migration/migration-0-28.md") == 0
    assert num_errors("As of 0.43, this is the default.\n", filepath="docs/content/changelog/upcoming/x.md") == 0

    # `py` code fences.
    assert num_errors("```py\nx = 1\n```\n") == 1
    assert num_errors("```python\nx = 1\n```\n") == 0

    # TODOs that readers see.
    assert num_errors("TODO(#1150): Allow disabling this.\n") == 1
    assert num_errors("<!-- TODO(#1150): Allow disabling this. -->\n") == 0
    assert num_errors("<!--\nTODO(#1150): Allow disabling this.\n-->\n") == 0
    assert num_errors("<!-- a --> TODO(#1150) <!-- b -->\n") == 1
    assert lint_docs_page("docs/content/a.md", "<!-- TODO(a) --> TODO(b)\n")[0][0] == 17

    # Readers never see HTML comments.
    assert num_errors("<!-- See https://rerun.io/docs/viewer. -->\n") == 0
    assert num_errors("<!--\nSince Rerun 0.28, this works.\n-->\n") == 0
    assert num_errors("TODO(name): add docs link\n", filepath="docs/content/changelog/upcoming/x.md") == 0
    assert num_errors("```html\n<!-- start of embed\n```\nTODO(#1)\n") == 1

    # Hidden redirect pages.
    assert num_errors("---\ntitle: Old\nhidden: true\nredirect: viewer/new\n---\n") == 1
    assert num_errors("---\ntitle: Index\nredirect: getting-started\n---\n") == 0
    assert num_errors("---\ntitle: Page\nhidden: true\n---\n") == 0


def test_lint_example_app_ids() -> None:
    def app_id_errors(content: str) -> list[str]:
        return [error for _, error in lint_example_app_ids(content)]

    for content in [
        'rr.init("rerun_example_app")',
        'rr.script_setup(args, "rerun_example_app")',
        'rr.script_setup(\n    args,\n    "rerun_example_app",\n)',
        'RecordingStreamBuilder::new(\n    "rerun_example_app",\n)',
        'rr.init("<your_app_name>")',
    ]:
        assert app_id_errors(content) == [], content

    for content in [
        'RecordingStreamBuilder::new("missing_prefix")',
        'args.rerun.init("missing_prefix")',
        'RecordingStream("missing_prefix")',
        'rr.init("missing_prefix")',
        'rr.script_setup(args, "missing_prefix")',
        'rr.script_setup(args, "")',
        'rr.script_setup(\n    args,\n    "missing_prefix",\n)',
        'rr.RecordingStream(\n    "missing_prefix",\n)',
    ]:
        assert len(app_id_errors(content)) == 1, content


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


def header_casing_error(kind: str, old: str, new: str) -> str:
    """Explain a casing fix: lowercasing title-cased words, capitalizing always-capitalized words, or both."""
    old_words = old.split()
    new_words = new.split()
    if len(old_words) != len(new_words):
        return f"{kind} have the wrong casing. This should be '{new}'."

    changed = [(o, n) for o, n in zip(old_words, new_words, strict=True) if o != n]
    lowered = [o for o, n in changed if n[:1].islower()]
    raised = [n for o, n in changed if not n[:1].islower()]

    reasons = []
    if lowered:
        reasons.append(f"should NOT be title cased ({', '.join(lowered)})")
    if raised:
        reasons.append(f"should use the canonical casing of {', '.join(raised)}")
    if not reasons:
        return f"{kind} have the wrong casing. This should be '{new}'."
    return f"{kind} {' and '.join(reasons)}. This should be '{new}'."


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

        if (
            not in_code_block
            and not source.should_ignore(line_nr)
            and "docs/content/" in filepath
            and "<img" in line
            and not re.search(r'alt="[^"]+"', line)
        ):
            errors.append(
                f'{line_nr}: Images in the docs need alt text: add `alt="…"` saying what the image shows.',
            )

        if not in_code_block and not source.should_ignore(line_nr) and filepath.startswith(rerun_prefix):
            if not in_metadata:
                # Check the casing on markdown headers
                if m := re.match(r"(\#+ )(.*)", line):
                    new_header = fix_header_casing(m.group(2))
                    if new_header != m.group(2):
                        errors.append(
                            f"{line_nr}: {header_casing_error('Markdown headers', m.group(2), new_header)}",
                        )
                        line = m.group(1) + new_header + "\n"

            # Check the casing on `title = "…"` frontmatter
            elif m := re.match(r'title\s*\=\s*"(.*)"', line):
                new_title = fix_header_casing(m.group(1))
                if new_title != m.group(1):
                    errors.append(
                        f"{line_nr}: {header_casing_error('Titles', m.group(1), new_title)}",
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

    if not filepath.startswith(f"{rerun_prefix}examples/python") or not filepath.endswith("README.md"):
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


def _index_to_line_nr(content: str, index: int) -> int:
    """Converts a 0-based index into a 0-based line number."""
    return content[:index].count("\n")


class SourceFile:
    """Wrapper over a source file with some utility functions."""

    def __init__(self, path: str, content: str | None = None) -> None:
        self.path = path
        self.ext = path.rsplit(".", maxsplit=1)[-1]
        if content is None:
            with open(path, encoding="utf8") as f:
                self.lines = f.readlines()
        else:
            self.lines = content.splitlines(keepends=True)
        self._update_content()

    def _update_content(self) -> None:
        """Sync everything with `self.lines`."""
        self.content = "".join(self.lines)

        # The 0-based lines with a `NOLINT` marker, or inside a `NOLINT_START`/`NOLINT_END` block.
        self.nolints: set[int] = set()
        is_in_nolint_block = False
        for i, line in enumerate(self.lines):
            if "NOLINT_START" in line:
                is_in_nolint_block = True
            if is_in_nolint_block or has_nolint(line):
                self.nolints.add(i)
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

    def should_ignore(self, from_line: int, to_line: int | None = None) -> bool:
        """
        Determines if we should ignore a violation.

        NOLINT might be on the same line(s) as the violation or the previous line.

        Args:
            from_line: Starting line number (1-based)
            to_line: Ending line number (1-based), defaults to from_line

        """

        if to_line is None:
            to_line = from_line

        return any(i in self.nolints for i in range(from_line - 1, to_line + 1))

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
            return f"{self.path}:{line_nr + 1}: {message}"


def lint_file(filepath: str, args: Any, content: str | None = None) -> int:
    """
    Lint a file, print the errors, and return how many there were.

    `content` replaces the contents of the file on disk. It is used to find dead `NOLINT`s.
    """
    source = SourceFile(filepath, content)
    num_errors = 0

    error: str | None

    is_in_docstring = False
    is_in_oss_rerun_repo = filepath.startswith(rerun_prefix)

    docstring_lines = set()
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
            if is_in_docstring:
                docstring_lines.add(line_nr)
            error = lint_line(line, prev_line, source.ext, is_in_docstring, is_in_oss_rerun_repo)
            prev_line = line
        if error is not None:
            num_errors += 1
            print(source.error(error, line_nr=line_nr))

    if filepath.endswith(".hpp"):
        if not any(line.startswith("#pragma once") for line in source.lines):
            print(source.error("Missing `#pragma once` in C++ header file"))
            num_errors += 1

    if filepath.endswith(".rs"):
        for match in debug_formatted_ui_label.finditer(source.content):
            line_nr = _index_to_line_nr(source.content, match.start())
            if not source.should_ignore(line_nr):
                print(source.error("Use Display, not Debug formatting, in GUI labels", line_nr=line_nr))
                num_errors += 1

    if filepath.endswith(".proto"):
        for line_nr, line in enumerate(source.lines):
            if source.should_ignore(line_nr):
                continue
            if triple_slash_comment.search(line):
                print(source.error("Use `//` not `///` for comments in .proto files", line_nr=line_nr))
                num_errors += 1

        # `buf lint` has a COMMENTS category for messages, fields, RPCs and so on, but no rule for
        # the file itself, so we check that here: every .proto starts with a comment saying what
        # lives in it.
        if not source.should_ignore(0) and not (source.lines and source.lines[0].startswith("//")):
            print(source.error("A .proto file must start with a comment explaining what it defines", line_nr=0))
            num_errors += 1

    if filepath.endswith(".md"):
        errors, lines_out = lint_markdown(filepath, source)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

        if args.fix:
            source.rewrite(lines_out)
        elif 0 < num_errors:
            print(f"Run with --fix to automatically fix {num_errors} errors.")

    if filepath.endswith("Cargo.toml") and not filepath.startswith(f"{rerun_prefix}examples/rust"):
        is_workspace = "[workspace]" in source.content
        error = (
            lint_workspace_dependency_headings(source.content) if is_workspace else lint_workspace_lints(source.content)
        )

        if error is not None:
            print(source.error(error))
            num_errors += 1

    for index, error in lint_example_app_ids(source.content):
        line_nr = _index_to_line_nr(source.content, index)
        if line_nr not in docstring_lines and not source.should_ignore(line_nr):
            print(source.error(error, line_nr=line_nr))
            num_errors += 1

    for index, error in lint_docs_redirect_links(filepath, source.content, redirected_doc_paths()):
        if not source.should_ignore_index(index):
            print(source.error(error, line_nr=_index_to_line_nr(source.content, index)))
            num_errors += 1

    for index, error in lint_docs_page(filepath, source.content):
        if not source.should_ignore_index(index):
            print(source.error(error, line_nr=_index_to_line_nr(source.content, index)))
            num_errors += 1

    # Markdown-specific lints
    if filepath.endswith(".md"):
        errors = lint_frontmatter(filepath, source.content)

        for error in errors:
            print(source.error(error))
        num_errors += len(errors)

    if content is None and not args.fix:
        for error in find_dead_nolints(source, args):
            print(error)
            num_errors += 1

    return num_errors


# A `NOLINT` right after a backtick is a mention in prose, like this one, not a marker.
nolint_marker = re.compile(r"(?<!`)NOLINT")
nolint_comment = re.compile(r"[ \t]*(<!--\s*NOLINT.*?-->|/\*\s*NOLINT.*?\*/|(//|#)\s*NOLINT.*?(?=\n|$))")


def has_nolint(line: str) -> bool:
    return nolint_marker.search(line) is not None


def without_nolint(line: str) -> str:
    """The line as it would be without its `NOLINT` comment, so the comment syntax itself can't hide an error."""
    stripped = nolint_comment.sub("", line, count=1)
    return stripped if stripped != line else nolint_marker.sub("N0LINT", line)


def find_dead_nolints(source: SourceFile, args: Any) -> list[str]:
    """
    Find the `NOLINT`s that suppress nothing.

    A `NOLINT` suppresses errors on its own line and the next one.
    A dead `NOLINT` would hide real problems that later appear there, so it is an error.
    """
    if source.ext in ("c", "cpp", "h", "hpp"):
        return []  # clang-tidy uses `NOLINT` markers too
    if "CHANGELOG.md" in source.path or "docs/content/changelog/" in source.path:
        return []  # History: it mentions NOLINT, and quotes old code.

    markers = [i for i, line in enumerate(source.lines) if has_nolint(line)]
    if not markers:
        return []

    output = StringIO()
    quiet_args = argparse.Namespace(**{**vars(args), "fix": False})
    with redirect_stdout(output):
        lint_file(source.path, quiet_args, content="".join(without_nolint(line) for line in source.lines))
    prefix = f"{source.path}:"
    error_lines = {
        int(line_nr) - 1
        for line_nr, _ in (
            line[len(prefix) :].split(":", 1) for line in output.getvalue().splitlines() if line.startswith(prefix)
        )
        if line_nr.isdigit()
    }

    errors = []
    block_start = None
    for i in markers:
        line = source.lines[i]
        if "NOLINT_START" in line:
            block_start = i
            continue
        if "NOLINT_END" in line and block_start is not None:
            used = any(block_start <= nr <= i for nr in error_lines)
            nr, block_start = block_start, None
        else:
            used = i in error_lines or i + 1 in error_lines
            nr = i
        if not used:
            errors.append(source.error("This NOLINT does not suppress anything. Remove it.", line_nr=nr))
    return errors


def test_find_dead_nolints() -> None:
    args = argparse.Namespace(fix=False)

    def dead_lines(content: str, extension: str = "txt") -> list[int]:
        source = SourceFile(f"{rerun_prefix}test.{extension}", content)
        return [int(error.split(":")[1]) for error in find_dead_nolints(source, args)]

    assert dead_lines("FIXME # NOLINT\n") == []
    assert dead_lines("all good # NOLINT\n") == [1]
    assert dead_lines("# NOLINT\nFIXME\n") == [], "a NOLINT covers the next line too"
    assert dead_lines("FIXME # NOLINT\nHACK # NOLINT\n") == [], "each covers its own line"
    assert dead_lines("# NOLINT_START\nFIXME\n# NOLINT_END\n") == []
    assert dead_lines("# NOLINT_START\nfine\n# NOLINT_END\n") == [1]
    assert dead_lines("This mentions `NOLINT` in prose.\n") == []
    assert dead_lines("Use `...` <!-- NOLINT -->\n") == [], "the comment itself must not hide the error"
    assert dead_lines("x(); // NOLINT\n", "cpp") == [], "clang-tidy uses NOLINT too"


_worker_args: argparse.Namespace | None = None


def _init_lint_worker(args: argparse.Namespace, worker_rerun_prefix: str) -> None:
    global _worker_args, rerun_prefix
    _worker_args = args
    rerun_prefix = worker_rerun_prefix


def _lint_file_worker(filepath: str) -> tuple[int, str]:
    assert _worker_args is not None
    output = StringIO()
    with redirect_stdout(output):
        num_errors = lint_file(filepath, _worker_args)
    return num_errors, output.getvalue()


def lint_files(filepaths: list[str], args: argparse.Namespace) -> int:
    if args.jobs == 1 or len(filepaths) < 2:
        return sum(lint_file(filepath, args) for filepath in filepaths)

    num_errors = 0
    printed_anything = False
    try:
        with ProcessPoolExecutor(
            max_workers=min(args.jobs, len(filepaths)),
            initializer=_init_lint_worker,
            initargs=(args, rerun_prefix),
        ) as executor:
            for file_errors, output in executor.map(_lint_file_worker, filepaths, chunksize=16):
                print(output, end="")
                printed_anything = True
                num_errors += file_errors
    except PermissionError:
        # A process pool needs POSIX semaphores, which some sandboxes deny.
        # Linting one file at a time needs none, so fall back to that.
        if printed_anything:
            raise
        print("Cannot lint in parallel here, falling back to `--jobs 1`.", file=sys.stderr)
        return sum(lint_file(filepath, args) for filepath in filepaths)

    return num_errors


pixi_version_pin = re.compile(r"(pixi-version:\s*[\"']?v?|PIXI_VERSION = \"|requires-pixi = \">=)([\d.]+)")


def lint_pixi_version_pins(repo: git.Repo) -> int:
    """
    Check that every pixi version pin matches `requires-pixi` in `rerun/pixi.toml`.

    Pins live in CI workflows, `pixi.toml` files, and `check_env.py`.
    This checks all tracked files, ignoring `exclude_paths`, so excluded workflows are checked too.
    """
    pixi_toml = Path(f"{rerun_prefix}pixi.toml").read_text("utf-8")
    required_match = re.search(r'^requires-pixi = ">=([\d.]+)"', pixi_toml, re.MULTILINE)
    assert required_match is not None, "rerun/pixi.toml has no `requires-pixi`"
    required = required_match.group(1)

    num_errors = 0
    for item in repo.index.iter_blobs():
        filepath = "./" + str(item[1].path).replace("\\", "/")
        if not (filepath.endswith(("/pixi.toml", "/check_env.py")) or "/.github/workflows/" in filepath):
            continue
        content = Path(filepath).read_text("utf-8")
        for pin in pixi_version_pin.finditer(content):
            if pin.group(2) != required:
                line_nr = content.count("\n", 0, pin.start()) + 1
                print(
                    f"{filepath}:{line_nr}: pixi version {pin.group(2)} differs from `requires-pixi` in rerun/pixi.toml ({required})"
                )
                num_errors += 1
    return num_errors


def lint_crate_docs() -> int:
    """Make sure ARCHITECTURE.md talks about every single crate we have."""

    crates_dir = Path(f"{rerun_prefix}crates")
    architecture_md_file = Path(f"{rerun_prefix}ARCHITECTURE.md")

    architecture_md = architecture_md_file.read_text("utf-8")

    # extract all crate names ("re_…") from ARCHITECTURE.md to ensure they actually exist
    listed_crates: dict[str, int] = {}
    for i, line in enumerate(architecture_md.split("\n"), start=1):
        for crate_name in re.findall(r"\bre_\w+", line):
            if crate_name not in listed_crates:
                listed_crates[crate_name] = i

    error_count = 0
    for cargo_toml in crates_dir.glob("**/Cargo.toml"):
        # The crate name is not always the directory name.
        package_name = re.search(r'^name = "(.+)"$', cargo_toml.read_text("utf-8"), re.MULTILINE)
        crate_name = package_name.group(1) if package_name else cargo_toml.parent.name

        listed_crates.pop(crate_name, None)

        if not re.search(r"\b" + crate_name + r"\b", architecture_md):
            print(f"{architecture_md_file}: missing documentation for crate {crate_name}")
            error_count += 1

    for crate_name, line_nr in sorted(listed_crates.items(), key=lambda x: x[1]):
        print(f"{architecture_md_file}:{line_nr}: crate name {crate_name} does not exist")
        error_count += 1

    return error_count


def main() -> None:
    # Make sure we are bug free before we run:
    test_split_words()
    test_lint_line()
    test_is_emoji()
    test_lint_docs_redirect_links()
    test_lint_docs_page()
    test_lint_example_app_ids()

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
    parser.add_argument(
        "--jobs",
        type=int,
        default=os.cpu_count() or 1,
        help="Number of files to lint in parallel. Use 1 to disable parallel processing.",
    )

    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be at least 1")

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
        "mjs",
        "proto",
        "py",
        "rs",
        "sh",
        "toml",
        "ts",
        "txt",
        "wgsl",
        "yaml",
        "yml",
    ]

    rerun_root = get_rerun_root()

    # Find the git root. In the monorepo (reality), it's the parent of rerun_root.
    # In the standalone rerun repo, it IS rerun_root.
    repo = git.Repo(rerun_root, search_parent_directories=True)
    assert repo.working_tree_dir is not None, "Expected a non-bare git repository"
    repo_root = repo.working_tree_dir

    # Resolve explicit file arguments against the invocation directory, before we chdir below.
    args.files = [os.path.abspath(filepath) for filepath in args.files]
    os.chdir(repo_root)

    # Path prefix from git root to the rerun directory.
    # Monorepo: "./rerun/", Standalone: "./"
    global rerun_prefix
    rerun_relative = Path(rerun_root).relative_to(repo_root)
    rerun_prefix = str(rerun_relative).replace("\\", "/")
    if rerun_prefix == ".":
        rerun_prefix = "./"
    else:
        rerun_prefix = "./" + rerun_prefix + "/"

    # Helper to build a path relative to the git root, inside the rerun directory.
    def rerun(path: str) -> str:
        return f"{rerun_prefix}{path}"

    test_find_dead_nolints()  # needs the working directory, for `_redirects.yaml`

    exclude_paths = (
        "./dataplatform/crates/redap_protos/Cargo.toml",  # intentional [lints.clippy] override (see file header)
        "./dataplatform/crates/redap_protos/src/v1alpha1",  # auto-generated
        rerun(".github/workflows/reusable_checks.yml"),  # zombie TODO hunting job
        rerun(".nox"),
        rerun(".pytest_cache"),
        rerun("CODE_STYLE.md"),
        rerun("crates/build/re_types_builder/src/codegen/rust/reflection.rs"),  # auto-generated
        rerun("crates/store/re_protos/src/v1alpha1"),  # auto-generated
        rerun("crates/ui_tools/re_ui/data/Inter-README.txt"),  # third-party font readme (Inter)
        rerun("crates/top/re_web_viewer_server/web_viewer/re_viewer.js"),  # auto-generated by wasm_bindgen
        rerun("docs/content/_redirects.yaml"),  # only contains URLs, which are always lowercase
        rerun("docs/content/concepts/app-model.md"),  # this really needs custom letter casing
        rerun("docs/content/reference/cli.md"),  # auto-generated
        rerun("docs/snippets/all/tutorials/custom-application-id.cpp"),  # nuh-uh, I don't want rerun_example_ here
        rerun("docs/snippets/all/tutorials/custom-application-id.py"),  # nuh-uh, I don't want rerun_example_ here
        rerun("docs/snippets/all/tutorials/custom-application-id.rs"),  # nuh-uh, I don't want rerun_example_ here
        rerun("examples/assets"),
        rerun("examples/python/detect_and_track_objects/cache/version.txt"),
        rerun("examples/python/objectron/objectron/proto/"),  # auto-generated
        rerun("examples/rust/objectron/src/objectron.rs"),  # auto-generated
        rerun("rerun_cpp/docs/doxygen-awesome/"),  # copied from an external repository
        rerun("rerun_cpp/docs/html"),
        rerun("rerun_cpp/src/rerun/c/arrow_c_data_interface.h"),  # Not our code
        rerun("rerun_cpp/src/rerun/third_party/cxxopts.hpp"),  # vendored
        rerun("rerun_js/docs/"),  # auto-generated
        rerun("rerun_js/node_modules"),
        rerun("rerun_js/web-viewer-react/node_modules"),
        rerun("rerun_js/web-viewer/index.js"),
        rerun("rerun_js/web-viewer/inlined.js"),
        rerun("rerun_js/web-viewer/node_modules"),
        rerun("rerun_js/web-viewer/re_viewer_bg.js"),  # auto-generated by wasm_bindgen
        rerun("rerun_js/web-viewer/re_viewer.js"),
        rerun("rerun_notebook/node_modules"),
        rerun("rerun_notebook/src/rerun_notebook/static"),
        rerun("rerun_py/.pytest_cache/"),
        rerun("rerun_py/site/"),  # is in `.gitignore` which this script doesn't fully respect
        rerun("scripts/lint.py"),  # we contain all the patterns we are linting against
        rerun("scripts/zombie_todos.py"),
        rerun("tests/assets/lerobot/apple_storage/README.md"),  # not ours
        rerun("tests/python/gil_stress/main.py"),
        rerun("tests/python/release_checklist/main.py"),
    )

    should_ignore = parse_gitignore(".gitignore")  # TODO(#6730): parse all .gitignore files, not just top-level

    files_to_lint: list[str] = []
    if args.files:
        for filepath in args.files:
            filepath = os.path.join(".", os.path.relpath(filepath, repo_root))
            filepath = str(filepath).replace("\\", "/")
            extension = filepath.split(".")[-1]
            if extension in extensions and not should_ignore(filepath) and not filepath.startswith(exclude_paths):
                files_to_lint.append(filepath)
    else:
        tracked_files = [str(item[1].path) for item in repo.index.iter_blobs()]
        for filepath in tracked_files:
            filepath = "./" + filepath
            filepath = filepath.replace("\\", "/")

            # In the standalone rerun repo `rerun_prefix` is "./" and we lint everything.
            # In the monorepo (reality) we lint everything except a few directories
            # with their own tooling (e.g. `./landing/`), so that all top-level
            # directories and files share the same custom lints.
            monorepo_skipped_prefixes = ("./landing/",)
            if rerun_prefix != "./" and filepath.startswith(monorepo_skipped_prefixes):
                continue

            extension = filepath.split(".")[-1]
            if extension in extensions and not filepath.startswith(exclude_paths):
                files_to_lint.append(filepath)

    num_errors += lint_files(files_to_lint, args)
    num_errors += lint_pixi_version_pins(repo)

    if not args.files:
        num_errors += lint_crate_docs()

    if num_errors == 0:
        print(f"{sys.argv[0]} finished without error")
        sys.exit(0)
    else:
        print(f"{sys.argv[0]} found {num_errors} errors.")
        sys.exit(1)


if __name__ == "__main__":
    main()
