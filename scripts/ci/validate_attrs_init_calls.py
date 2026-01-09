#!/usr/bin/env python3
"""
Static analysis tool to validate that __attrs_init__ calls in extension classes
pass all available arguments from the target class.

This tool uses AST parsing to analyze source code without executing it.
"""

import ast
import inspect
import sys
from pathlib import Path
from typing import List, Set, Optional, Tuple
from dataclasses import dataclass


@dataclass
class MethodCall:
    """Represents a method call found in source code."""

    method_name: str
    arguments: Set[str]
    line_number: int
    file_path: str


@dataclass
class ValidationResult:
    """Result of validating a method call."""

    file_path: str
    line_number: int
    method_name: str
    missing_args: List[str]
    extra_args: List[str]
    all_args_provided: bool


class AttrsInitCallVisitor(ast.NodeVisitor):
    """AST visitor to find __attrs_init__ method calls."""

    def __init__(self, file_path: Path):
        self.file_path = file_path
        self.calls: List[MethodCall] = []

    def visit_Call(self, node: ast.Call) -> None:
        # Look for self.__attrs_init__(...) calls
        if (
            isinstance(node.func, ast.Attribute)
            and node.func.attr == "__attrs_init__"
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "self"
        ):
            # Extract keyword arguments
            kwargs = set()
            for keyword in node.keywords:
                if keyword.arg:  # Skip **kwargs
                    kwargs.add(keyword.arg)

            self.calls.append(
                MethodCall(
                    method_name="__attrs_init__",
                    arguments=kwargs,
                    line_number=node.lineno,
                    file_path=str(self.file_path),
                )
            )

        self.generic_visit(node)


class DelegationVisitor(ast.NodeVisitor):
    """AST visitor to detect class delegation patterns."""

    def __init__(self, target_class_name: str):
        self.target_class_name = target_class_name
        self.delegates_to: Optional[Tuple[str, str]] = None

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        if node.name == self.target_class_name:
            # First check base classes for datatype inheritance
            for base in node.bases:
                if (
                    isinstance(base, ast.Attribute)
                    and isinstance(base.value, ast.Name)
                    and base.value.id == "datatypes"
                ):
                    self.delegates_to = ("datatypes", base.attr)
                    return

            # Also look for comments indicating delegation
            for child in ast.walk(node):
                if (
                    isinstance(child, ast.Expr)
                    and isinstance(child.value, ast.Constant)
                    and isinstance(child.value.value, str)
                    and "delegates to" in child.value.value
                ):
                    # Extract the delegated class name from comment
                    comment = child.value.value
                    if "delegates to datatypes." in comment:
                        # Extract class name after "datatypes."
                        import re

                        match = re.search(r"delegates to datatypes\.(\w+)", comment)
                        if match:
                            self.delegates_to = ("datatypes", match.group(1))
                            return

        self.generic_visit(node)


class AttrsFieldVisitor(ast.NodeVisitor):
    """AST visitor to extract attrs field definitions from a class."""

    def __init__(self, target_class_name: str):
        self.target_class_name = target_class_name
        self.attrs_params: Set[str] = set()

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        if node.name == self.target_class_name:
            # Look for field assignments and __attrs_clear__ method
            for item in node.body:
                if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
                    # Look for field definitions like: field_name: SomeType = field(...)
                    if (
                        isinstance(item.value, ast.Call)
                        and isinstance(item.value.func, ast.Name)
                        and item.value.func.id == "field"
                    ):
                        self.attrs_params.add(item.target.id)
                elif isinstance(item, ast.FunctionDef) and item.name == "__attrs_clear__":
                    # Extract parameters from __attrs_clear__ call
                    self._extract_from_attrs_clear(item)

        self.generic_visit(node)

    def _extract_from_attrs_clear(self, func_node: ast.FunctionDef) -> None:
        """Extract parameters from __attrs_clear__ method."""
        for stmt in func_node.body:
            if (
                isinstance(stmt, ast.Expr)
                and isinstance(stmt.value, ast.Call)
                and isinstance(stmt.value.func, ast.Attribute)
                and stmt.value.func.attr == "__attrs_init__"
            ):
                # Found self.__attrs_init__(...) call
                for keyword in stmt.value.keywords:
                    if keyword.arg:
                        self.attrs_params.add(keyword.arg)


class AttrsInitValidator:
    """Validates __attrs_init__ calls against actual class definitions."""

    def __init__(self, source_root: Path):
        self.source_root = source_root
        self.results: List[ValidationResult] = []

    def find_attrs_init_calls(self, file_path: Path) -> List[MethodCall]:
        """Find all __attrs_init__ calls in a Python file."""
        source_code = self._read_file(file_path)
        if not source_code:
            return []

        tree = self._parse_file(file_path, source_code)
        if not tree:
            return []

        visitor = AttrsInitCallVisitor(file_path)
        visitor.visit(tree)
        return visitor.calls

    def get_attrs_init_signature(self, cls: type) -> Optional[Set[str]]:
        """Get the parameter names of a class's __attrs_init__ method."""
        try:
            attrs_init = getattr(cls, "__attrs_init__", None)
            if not attrs_init:
                return None

            sig = inspect.signature(attrs_init)
            params = set()

            for name, param in sig.parameters.items():
                if name != "self":
                    params.add(name)

            return params
        except Exception as e:
            print(f"Warning: Could not get signature for {cls}: {e}")
            return None

    def find_target_class(self, extension_file: Path) -> Optional[Tuple[Path, str]]:
        """
        Find the target class file and name that the extension extends.
        Handles both direct classes and delegation patterns.
        """
        if not extension_file.name.endswith("_ext.py"):
            return None

        # Get the base name (e.g., pinhole_ext.py -> pinhole)
        base_name = extension_file.name.replace("_ext.py", "")

        # Determine class name (e.g., pinhole -> Pinhole)
        class_name = base_name.replace("_", " ").title().replace(" ", "")

        # First, try the same directory (components or archetypes)
        target_file = extension_file.parent / f"{base_name}.py"

        if target_file.exists():
            # Check if this class delegates to a datatype
            delegation_info = self.check_for_delegation(target_file, class_name)
            if delegation_info:
                return delegation_info
            else:
                return (target_file, class_name)

        return None

    def check_for_delegation(self, class_file: Path, class_name: str) -> Optional[Tuple[Path, str]]:
        """
        Check if a class delegates to another class (e.g., component -> datatype).
        Returns the delegated class info if found.
        """
        source_code = self._read_file(class_file)
        if not source_code:
            return None

        tree = self._parse_file(class_file, source_code)
        if not tree:
            return None

        visitor = DelegationVisitor(class_name)
        visitor.visit(tree)

        if visitor.delegates_to:
            module_type, delegated_class_name = visitor.delegates_to
            if module_type == "datatypes":
                # Look for the datatype file
                datatypes_dir = self.find_datatypes_directory(class_file)
                if not datatypes_dir:
                    print(f"Warning: Could not find datatypes directory for {class_file}")
                    return None

                datatype_file = self.find_datatype_file(datatypes_dir, delegated_class_name)

                if datatype_file and datatype_file.exists():
                    return (datatype_file, delegated_class_name)
                else:
                    print(f"Warning: Could not find datatype file for {delegated_class_name} in {datatypes_dir}")

        return None

    def _read_file(self, file_path: Path) -> Optional[str]:
        """Helper method to read file contents."""
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                return f.read()
        except Exception as e:
            print(f"Warning: Could not read {file_path}: {e}")
            return None

    def _parse_file(self, file_path: Path, source_code: str) -> Optional[ast.AST]:
        """Helper method to parse Python source code."""
        try:
            return ast.parse(source_code)
        except SyntaxError as e:
            print(f"Warning: Syntax error in {file_path}: {e}")
            return None

        return None

    def find_datatypes_directory(self, class_file: Path) -> Optional[Path]:
        """Find the appropriate datatypes directory for a given class file."""
        # For blueprint components: rerun_py/rerun_sdk/rerun/blueprint/components/foo.py
        # -> datatypes at: rerun_py/rerun_sdk/rerun/datatypes/

        # For regular components: rerun_py/rerun_sdk/rerun/components/foo.py
        # -> datatypes at: rerun_py/rerun_sdk/rerun/datatypes/

        # Go up directories until we find the rerun package root
        current = class_file.parent
        while current.name != "rerun" and current.parent != current:
            current = current.parent

        if current.name == "rerun":
            datatypes_dir = current / "datatypes"
            if datatypes_dir.exists():
                return datatypes_dir

        return None

    def find_datatype_file(self, datatypes_dir: Path, class_name: str) -> Optional[Path]:
        """Find the datatype file for a given class name, trying various naming conventions."""

        # Try different naming patterns
        patterns_to_try = [
            # Direct lowercase
            class_name.lower(),
            # CamelCase to snake_case
            self.camel_to_snake(class_name),
        ]

        for pattern in patterns_to_try:
            candidate_file = datatypes_dir / f"{pattern}.py"
            if candidate_file.exists():
                return candidate_file

        # If none found, list what's available for debugging
        available_files = [f.stem for f in datatypes_dir.glob("*.py") if not f.stem.startswith("_")]
        print(f"Warning: Could not find datatype file for {class_name}. Available: {available_files}")

        return None

    def camel_to_snake(self, name: str) -> str:
        """Convert CamelCase to snake_case."""
        import re

        # Handle cases like Range2D -> range2d, not range_2_d
        s1 = re.sub("([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
        s2 = re.sub("([a-z0-9])([A-Z])", r"\1_\2", s1)
        return s2.lower().replace("_2_d", "2d").replace("_3_d", "3d")  # NOLINT: 2d/3d ok for filenames

    def get_attrs_init_signature_from_ast(self, file_path: Path, class_name: str) -> Optional[Set[str]]:
        """Extract __attrs_init__ signature from AST without importing."""
        source_code = self._read_file(file_path)
        if not source_code:
            return None

        tree = self._parse_file(file_path, source_code)
        if not tree:
            return None

        visitor = AttrsFieldVisitor(class_name)
        visitor.visit(tree)
        return visitor.attrs_params if visitor.attrs_params else None

    def validate_file(self, file_path: Path) -> List[ValidationResult]:
        """Validate all __attrs_init__ calls in a file."""
        calls = self.find_attrs_init_calls(file_path)
        if not calls:
            return []

        # Try to find the target class
        target_info = self.find_target_class(file_path)
        if not target_info:
            print(f"Warning: Could not find target class for {file_path}")
            return []

        target_file, class_name = target_info
        expected_params = self.get_attrs_init_signature_from_ast(target_file, class_name)

        if expected_params is None:
            print(f"Warning: Could not get __attrs_init__ signature for {class_name}")
            return []

        results = []
        for call in calls:
            missing_args = list(expected_params - call.arguments)
            extra_args = list(call.arguments - expected_params)

            result = ValidationResult(
                file_path=call.file_path,
                line_number=call.line_number,
                method_name=call.method_name,
                missing_args=missing_args,
                extra_args=extra_args,
                all_args_provided=len(missing_args) == 0,
            )
            results.append(result)

        return results

    def scan_directory(self, directory: Path, pattern: str = "*_ext.py") -> List[ValidationResult]:
        """Scan all matching files in a directory."""
        all_results: List[ValidationResult] = []

        # Find all matching files
        matching_files = list(directory.rglob(pattern))

        if not matching_files:
            print(f"No files matching '{pattern}' found in {directory}")
            return all_results

        print(f"Found {len(matching_files)} files matching '{pattern}':")

        for ext_file in sorted(matching_files):
            rel_path = ext_file.relative_to(self.source_root)
            print(f"  Analyzing {rel_path}…")
            results = self.validate_file(ext_file)
            all_results.extend(results)

        return all_results

    def print_results(self, results: List[ValidationResult]) -> None:
        """Print validation results in a readable format."""
        if not results:
            print("✅ No __attrs_init__ calls found or all calls are complete!")
            return

        issues_found = False

        for result in results:
            rel_path = Path(result.file_path).relative_to(self.source_root)

            if result.missing_args or result.extra_args:
                issues_found = True
                print(f"\n❌ Issues in {rel_path}:{result.line_number}")

                if result.missing_args:
                    print(f"   Missing arguments: {', '.join(sorted(result.missing_args))}")

                if result.extra_args:
                    print(f"   Extra arguments: {', '.join(sorted(result.extra_args))}")
            else:
                print(f"✅ {rel_path}:{result.line_number} - All arguments provided")

        if issues_found:
            print(f"\n❌ Found issues in {sum(1 for r in results if r.missing_args or r.extra_args)} call(s)")
            sys.exit(1)
        else:
            print(f"\n✅ All {len(results)} __attrs_init__ call(s) are complete!")

    def print_results_ci(self, results: List[ValidationResult]) -> None:
        """Print validation results in CI-friendly format."""
        if not results:
            print("No __attrs_init__ calls found")
            return

        issues = [r for r in results if r.missing_args or r.extra_args]

        if issues:
            print(f"FAIL: Found {len(issues)} __attrs_init__ call(s) with issues:")
            for result in issues:
                rel_path = Path(result.file_path).relative_to(self.source_root)
                print(f"  {rel_path}:{result.line_number}")
                if result.missing_args:
                    print(f"    Missing: {', '.join(sorted(result.missing_args))}")
                if result.extra_args:
                    print(f"    Extra: {', '.join(sorted(result.extra_args))}")
            sys.exit(1)
        else:
            print(f"PASS: All {len(results)} __attrs_init__ call(s) complete")
            sys.exit(0)


def main() -> None:
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Validate __attrs_init__ calls in extension classes",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Scan all *_ext.py files in archetypes directory
  python validate_attrs_init_calls.py rerun_py/rerun_sdk/rerun/archetypes/

  # Validate a specific file
  python validate_attrs_init_calls.py --file rerun_py/rerun_sdk/rerun/archetypes/pinhole_ext.py

  # Scan with custom pattern
  python validate_attrs_init_calls.py --pattern "*_extension.py" some_directory/

  # For CI - scan archetypes and exit with error code if issues found
  python validate_attrs_init_calls.py rerun_py/rerun_sdk/rerun/archetypes/ --ci
        """,
    )
    parser.add_argument(
        "path",
        type=Path,
        nargs="?",
        default=Path("."),
        help="Path to scan for extension files (default: current directory)",
    )
    parser.add_argument("--file", type=Path, help="Validate a specific extension file instead of scanning")
    parser.add_argument(
        "--pattern", type=str, default="*_ext.py", help="File pattern to match when scanning (default: *_ext.py)"
    )
    parser.add_argument("--ci", action="store_true", help="CI mode: minimal output, exit with error code on issues")

    args = parser.parse_args()

    validator = AttrsInitValidator(args.path if not args.file else args.file.parent)

    if args.file:
        if not args.file.exists():
            print(f"Error: File {args.file} does not exist")
            sys.exit(1)
        if not args.ci:
            print(f"Validating {args.file.relative_to(Path.cwd()) if args.file.is_absolute() else args.file}…")
        results = validator.validate_file(args.file)
    else:
        if not args.path.exists():
            print(f"Error: Directory {args.path} does not exist")
            sys.exit(1)
        results = validator.scan_directory(args.path, args.pattern)

    if args.ci:
        validator.print_results_ci(results)
    else:
        validator.print_results(results)


if __name__ == "__main__":
    main()
