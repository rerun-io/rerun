#!/usr/bin/env python3

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

RERUN_ROOT = Path(__file__).absolute().parent.parent.parent

LIB_KINDS = {"lib", "rlib", "proc-macro", "cdylib", "dylib", "staticlib"}
BIN_KINDS = {"bin"}

TEST_ATTR_RE = re.compile(r"(?:^|_)test(?:_|$)|^rstest$")
RAW_STR_RE = re.compile(r'(?:b|c)?r(#*)"')
STR_RE = re.compile(r'(?:b|c)?"')
CHAR_RE = re.compile(r"b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]*\}|.)|[^\\'\n])'")
LIFETIME_RE = re.compile(r"'(?:r#)?[^\W\d]\w*")
IDENT_RE = re.compile(r"(?:r#)?[^\W\d]\w*")
NUMBER_RE = re.compile(r"\d\w*")

OPEN_TO_CLOSE = {"(": ")", "[": "]", "{": "}"}


@dataclass(frozen=True)
class Token:
    kind: str
    text: str
    line: int


@dataclass
class ModDecl:
    name: str
    line: int
    inline: list[str]
    path: str | None
    cfg_paths: list[str]


@dataclass
class Include:
    line: int
    path: str | None
    inline: list[str]


@dataclass
class MacroDef:
    has_test_attr: bool
    invokes: set[str]


@dataclass
class ParsedFile:
    test_attrs: list[tuple[int, str]] = field(default_factory=list)
    invocations: list[tuple[int, str]] = field(default_factory=list)
    macros: dict[str, MacroDef] = field(default_factory=dict)
    mods: list[ModDecl] = field(default_factory=list)
    includes: list[Include] = field(default_factory=list)


@dataclass
class TargetReport:
    tests: list[str] = field(default_factory=list)
    unsure: list[str] = field(default_factory=list)


def tokenize(src: str) -> list[Token]:
    tokens: list[Token] = []
    n = len(src)
    i = 0
    line = 1
    if src.startswith("#!") and not src.startswith("#!["):
        i = src.find("\n")
        if i == -1:
            i = n
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
            continue
        if c.isspace():
            i += 1
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j == -1 else j
            continue
        if src.startswith("/*", i):
            depth = 0
            while i < n:
                if src.startswith("/*", i):
                    depth += 1
                    i += 2
                elif src.startswith("*/", i):
                    depth -= 1
                    i += 2
                    if depth == 0:
                        break
                else:
                    if src[i] == "\n":
                        line += 1
                    i += 1
            continue
        m = RAW_STR_RE.match(src, i)
        if m:
            close = '"' + m.group(1)
            j = src.find(close, m.end())
            if j == -1:
                j = n
            value = src[m.end() : j]
            tokens.append(Token("str", value, line))
            line += value.count("\n")
            i = j + len(close)
            continue
        m = STR_RE.match(src, i)
        if m:
            j = m.end()
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            value = src[m.end() : j]
            tokens.append(Token("str", value, line))
            line += value.count("\n")
            i = j + 1
            continue
        if c == "'" or src.startswith("b'", i):
            m = CHAR_RE.match(src, i)
            if m:
                tokens.append(Token("lit", m.group(), line))
                i = m.end()
                continue
            m = LIFETIME_RE.match(src, i)
            if m:
                tokens.append(Token("lifetime", m.group(), line))
                i = m.end()
                continue
        m = IDENT_RE.match(src, i)
        if m:
            tokens.append(Token("ident", m.group(), line))
            i = m.end()
            continue
        m = NUMBER_RE.match(src, i)
        if m:
            tokens.append(Token("lit", m.group(), line))
            i = m.end()
            continue
        tokens.append(Token("punct", c, line))
        i += 1
    return tokens


def matching_close(tokens: list[Token], open_index: int) -> int:
    depth = 0
    for i in range(open_index, len(tokens)):
        tok = tokens[i]
        if tok.kind != "punct":
            continue
        if tok.text in OPEN_TO_CLOSE:
            depth += 1
        elif tok.text in (")", "]", "}"):
            depth -= 1
            if depth == 0:
                return i
    return len(tokens) - 1


def is_punct(tokens: list[Token], i: int, text: str) -> bool:
    return i < len(tokens) and tokens[i].kind == "punct" and tokens[i].text == text


def attribute_at(tokens: list[Token], i: int) -> tuple[int, int, bool] | None:
    if not is_punct(tokens, i, "#"):
        return None
    inner = is_punct(tokens, i + 1, "!")
    open_index = i + 2 if inner else i + 1
    if not is_punct(tokens, open_index, "["):
        return None
    return open_index + 1, matching_close(tokens, open_index), inner


def attr_path(attr: list[Token]) -> tuple[list[str], int]:
    path: list[str] = []
    i = 0
    while i < len(attr):
        tok = attr[i]
        if tok.kind == "ident":
            path.append(tok.text)
            i += 1
        elif is_punct(attr, i, ":") and is_punct(attr, i + 1, ":"):
            i += 2
        else:
            break
    return path, i


def split_top_level(tokens: list[Token]) -> list[list[Token]]:
    parts: list[list[Token]] = [[]]
    depth = 0
    for tok in tokens:
        if tok.kind == "punct" and tok.text in OPEN_TO_CLOSE:
            depth += 1
        elif tok.kind == "punct" and tok.text in (")", "]", "}"):
            depth -= 1
        if depth == 0 and tok.kind == "punct" and tok.text == ",":
            parts.append([])
        else:
            parts[-1].append(tok)
    return [part for part in parts if part]


def cfg_attr_children(attr: list[Token]) -> list[list[Token]] | None:
    path, end = attr_path(attr)
    if path != ["cfg_attr"] or not is_punct(attr, end, "("):
        return None
    close = matching_close(attr, end)
    return split_top_level(attr[end + 1 : close])[1:]


def is_test_attr(attr: list[Token]) -> bool:
    children = cfg_attr_children(attr)
    if children is not None:
        return any(is_test_attr(child) for child in children)
    path, _ = attr_path(attr)
    return bool(path) and TEST_ATTR_RE.search(path[-1].removeprefix("r#")) is not None


def path_value(attr: list[Token]) -> str | None:
    if len(attr) == 3 and attr[0].text == "path" and is_punct(attr, 1, "=") and attr[2].kind == "str":
        return attr[2].text
    return None


def attr_text(attr: list[Token]) -> str:
    out = ""
    for tok in attr:
        text = f'"{tok.text}"' if tok.kind == "str" else tok.text
        if out and (tok.kind != "punct" or tok.text == "=") and not out.endswith(("::", "(", "[")):
            out += " "
        out += text
    return f"#[{out}]"


def macro_invocation(tokens: list[Token], i: int) -> str | None:
    tok = tokens[i]
    if tok.kind != "ident" or tok.text == "macro_rules" or not is_punct(tokens, i + 1, "!"):
        return None
    if i + 2 < len(tokens) and tokens[i + 2].kind == "punct" and tokens[i + 2].text in OPEN_TO_CLOSE:
        return tok.text.removeprefix("r#")
    return None


def scan_tests_and_invocations(tokens: list[Token], start: int, end: int, parsed: ParsedFile) -> None:
    for i in range(start, end):
        attr = attribute_at(tokens, i)
        if attr is not None:
            body = tokens[attr[0] : attr[1]]
            if is_test_attr(body):
                parsed.test_attrs.append((tokens[i].line, attr_text(body)))
            continue
        name = macro_invocation(tokens, i)
        if name is not None:
            parsed.invocations.append((tokens[i].line, name))


def scan_macro_defs(tokens: list[Token], parsed: ParsedFile) -> None:
    for i, tok in enumerate(tokens):
        if not (tok.kind == "ident" and tok.text == "macro_rules" and is_punct(tokens, i + 1, "!")):
            continue
        if i + 3 >= len(tokens) or tokens[i + 2].kind != "ident":
            continue
        open_index = i + 3
        if not (tokens[open_index].kind == "punct" and tokens[open_index].text in OPEN_TO_CLOSE):
            continue
        close = matching_close(tokens, open_index)
        body = ParsedFile()
        scan_tests_and_invocations(tokens, open_index + 1, close, body)
        name = tokens[i + 2].text.removeprefix("r#")
        previous = parsed.macros.get(name, MacroDef(False, set()))
        parsed.macros[name] = MacroDef(
            previous.has_test_attr or bool(body.test_attrs),
            previous.invokes | {invoked for _, invoked in body.invocations},
        )


def scan_items(tokens: list[Token], parsed: ParsedFile) -> None:
    inline: list[tuple[str, int]] = []
    pending: list[list[Token]] = []
    depth = 0
    i = 0
    while i < len(tokens):
        tok = tokens[i]
        attr = attribute_at(tokens, i)
        if attr is not None:
            if not attr[2]:
                pending.append(tokens[attr[0] : attr[1]])
            i = attr[1] + 1
            continue
        if tok.kind == "ident" and tok.text == "pub":
            i += 1
            if is_punct(tokens, i, "("):
                i = matching_close(tokens, i) + 1
            continue
        if (
            tok.kind == "ident"
            and tok.text == "include"
            and is_punct(tokens, i + 1, "!")
            and is_punct(tokens, i + 2, "(")
        ):
            close = matching_close(tokens, i + 2)
            args = tokens[i + 3 : close]
            literal = args[0].text if len(args) == 1 and args[0].kind == "str" else None
            parsed.includes.append(Include(tok.line, literal, [name for name, _ in inline]))
            pending = []
            i = close + 1
            continue
        if (
            tok.kind == "ident"
            and tok.text == "mod"
            and i + 2 < len(tokens)
            and tokens[i + 1].kind == "ident"
            and tokens[i + 2].kind == "punct"
            and tokens[i + 2].text in (";", "{")
        ):
            name = tokens[i + 1].text.removeprefix("r#")
            path: str | None = None
            cfg_paths: list[str] = []
            for attr_tokens in pending:
                value = path_value(attr_tokens)
                if value is not None:
                    path = value
                for child in cfg_attr_children(attr_tokens) or []:
                    value = path_value(child)
                    if value is not None:
                        cfg_paths.append(value)
            pending = []
            if tokens[i + 2].text == ";":
                parsed.mods.append(ModDecl(name, tok.line, [component for component, _ in inline], path, cfg_paths))
            else:
                inline.append((path if path is not None else name, depth))
                depth += 1
            i += 3
            continue
        pending = []
        if tok.kind == "punct" and tok.text == "{":
            depth += 1
        elif tok.kind == "punct" and tok.text == "}":
            depth -= 1
            if inline and inline[-1][1] == depth:
                inline.pop()
        i += 1


def parse_file(src: str) -> ParsedFile:
    tokens = tokenize(src)
    parsed = ParsedFile()
    scan_tests_and_invocations(tokens, 0, len(tokens), parsed)
    scan_macro_defs(tokens, parsed)
    scan_items(tokens, parsed)
    return parsed


class Analyzer:
    def __init__(self, workspace_root: Path, tracked: set[Path] | None = None) -> None:
        self.workspace_root = workspace_root
        self.tracked = tracked
        self.parsed: dict[Path, ParsedFile] = {}
        self.target_files: dict[str, list[Path]] = {}
        self.target_unsure: dict[str, list[str]] = {}
        self.generators: set[str] = set()

    def display(self, path: Path) -> str:
        try:
            return str(path.relative_to(self.workspace_root))
        except ValueError:
            return str(path)

    def exists(self, path: Path) -> bool:
        return path.is_file() and (self.tracked is None or path.resolve() in self.tracked)

    def parse(self, path: Path) -> ParsedFile:
        if path not in self.parsed:
            self.parsed[path] = parse_file(path.read_text(encoding="utf-8"))
        return self.parsed[path]

    def walk(self, key: str, root: Path) -> None:
        files: list[Path] = []
        unsure: list[str] = []
        seen: set[tuple[Path, bool]] = set()
        stack: list[tuple[Path, bool]] = [(root.resolve(), True)]
        while stack:
            path, owns_dir = stack.pop()
            if (path, owns_dir) in seen:
                continue
            seen.add((path, owns_dir))
            if path not in files:
                files.append(path)
            parsed = self.parse(path)
            file_dir = path.parent
            mod_dir = file_dir if owns_dir else file_dir / path.stem
            for decl in parsed.mods:
                where = f"{self.display(path)}:{decl.line}"
                inline_dir = mod_dir.joinpath(*decl.inline)
                if decl.path is not None:
                    candidate = (inline_dir if decl.inline else file_dir) / decl.path
                    if self.exists(candidate):
                        stack.append((candidate.resolve(), True))
                    else:
                        unsure.append(f"{where}: cannot find `#[path = {decl.path!r}]` file for `mod {decl.name};`")
                    continue
                found = False
                for cfg_path in decl.cfg_paths:
                    candidate = (inline_dir if decl.inline else file_dir) / cfg_path
                    if self.exists(candidate):
                        stack.append((candidate.resolve(), True))
                        found = True
                if self.exists(inline_dir / f"{decl.name}.rs"):
                    stack.append(((inline_dir / f"{decl.name}.rs").resolve(), False))
                    found = True
                elif self.exists(inline_dir / decl.name / "mod.rs"):
                    stack.append(((inline_dir / decl.name / "mod.rs").resolve(), True))
                    found = True
                if not found:
                    unsure.append(f"{where}: cannot find the file for `mod {decl.name};`")
            for include in parsed.includes:
                where = f"{self.display(path)}:{include.line}"
                if include.path is None:
                    unsure.append(f"{where}: cannot resolve `include!` with a non-literal path")
                    continue
                candidate = file_dir / include.path
                if self.exists(candidate):
                    stack.append((candidate.resolve(), owns_dir))
                else:
                    unsure.append(f"{where}: cannot find `include!({include.path!r})`")
        self.target_files[key] = files
        self.target_unsure[key] = unsure

    def compute_generators(self) -> None:
        macros: dict[str, MacroDef] = {}
        for parsed in self.parsed.values():
            for name, macro in parsed.macros.items():
                previous = macros.get(name, MacroDef(False, set()))
                macros[name] = MacroDef(previous.has_test_attr or macro.has_test_attr, previous.invokes | macro.invokes)
        generators = {name for name, macro in macros.items() if macro.has_test_attr}
        changed = True
        while changed:
            changed = False
            for name, macro in macros.items():
                if name not in generators and macro.invokes & generators:
                    generators.add(name)
                    changed = True
        self.generators = generators

    def report(self, key: str) -> TargetReport:
        report = TargetReport(unsure=list(self.target_unsure[key]))
        for path in self.target_files[key]:
            parsed = self.parsed[path]
            for line, text in parsed.test_attrs:
                report.tests.append(f"{self.display(path)}:{line}: `{text}`")
            for line, name in parsed.invocations:
                if name in self.generators:
                    report.tests.append(f"{self.display(path)}:{line}: `{name}!` generates tests")
        return report


def cargo_metadata(root: Path) -> dict[str, Any]:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--no-deps"],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"cargo metadata failed with exit code {out.returncode}")
    metadata: dict[str, Any] = json.loads(out.stdout)
    return metadata


def git_tracked_files(root: Path) -> set[Path]:
    out = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        capture_output=True,
        check=True,
    )
    return {(root / name).resolve() for name in out.stdout.decode().split("\0") if name}


def section(target: dict[str, Any]) -> str:
    return "[lib]" if set(target["kind"]) & LIB_KINDS else "[[bin]]"


def check(metadata: dict[str, Any], tracked: set[Path] | None = None) -> tuple[list[str], int]:
    workspace_root = Path(metadata["workspace_root"])
    analyzer = Analyzer(workspace_root, tracked)

    checked: list[tuple[str, dict[str, Any], dict[str, Any]]] = []
    for package in metadata["packages"]:
        for target in package["targets"]:
            if not set(target["kind"]) & (LIB_KINDS | BIN_KINDS):
                continue
            key = f"{package['id']}#{target['name']}#{','.join(target['kind'])}"
            analyzer.walk(key, Path(target["src_path"]))
            checked.append((key, package, target))
    analyzer.compute_generators()

    errors: list[str] = []
    for key, package, target in checked:
        manifest = Path(package["manifest_path"]).relative_to(workspace_root)
        kind = "lib" if section(target) == "[lib]" else "bin"
        name = f"{kind} target `{target['name']}`"
        report = analyzer.report(key)
        if target["test"] and not report.tests and not report.unsure:
            errors.append(
                f"{manifest}: {name} contains no tests. Set `test = false` in its `{section(target)}` section."
            )
        elif not target["test"] and (report.tests or report.unsure):
            evidence = report.tests[0] if report.tests else f"possibly: {report.unsure[0]}"
            errors.append(
                f"{manifest}: {name} has `test = false`, so its tests never run ({evidence}). "
                f"Remove `test = false` from its `{section(target)}` section."
            )
    return errors, len(checked)


def main() -> int:
    errors, num_targets = check(cargo_metadata(RERUN_ROOT), git_tracked_files(RERUN_ROOT))

    if errors:
        print("Test target check failed:\n")
        for error in errors:
            print(f"  {error}")
        print()
        print("Every lib and bin target with `test = true` (the default) is linked into its own test binary,")
        print("even when it contains no tests. Set `test = false` on targets without tests,")
        print("and never on targets with tests, because those tests would silently never run.")
        return 1

    print(f"All {num_targets} lib and bin targets have `test` set to match whether they contain tests.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
