"""
Compares the signatures in `rerun_bindings.pyi` with the actual runtime signatures in `rerun_bindings.so`.

This does not check that the type annotations match. However, it does ensure that the number of arguments,
the argument names, and whether the arguments are required or have defaults match between the stub and runtime.
"""

from __future__ import annotations

import ast
import difflib
import importlib
import inspect
import sys
import textwrap
from inspect import Parameter, Signature
from pathlib import Path
from typing import Any

import parso
from colorama import Fore, Style, init as colorama_init

colorama_init()


def print_colored_diff(runtime: str, stub: str) -> None:
    # Split the strings into lines
    runtime_lines = runtime.splitlines()
    stub_lines = stub.splitlines()

    # Generate the diff
    diff = difflib.unified_diff(runtime_lines, stub_lines, fromfile="runtime", tofile="stub")

    # Print the diff output with colored lines
    for line in diff:
        if line.startswith("+"):
            print(Fore.GREEN + line + Style.RESET_ALL)
        elif line.startswith("-"):
            print(Fore.RED + line + Style.RESET_ALL)
        elif line.startswith("?"):
            print(Fore.YELLOW + line + Style.RESET_ALL)
        else:
            print(line)


class APIDef:
    def __init__(
        self, name: str, signature: Signature, internal_object: bool, doc: str | None, getset: bool = False
    ) -> None:
        self.name = name
        self.signature = signature
        self.internal_object = internal_object
        self.doc = inspect.cleandoc(doc) if doc else None
        self.getset = getset

    def __str__(self) -> str:
        doclines = (self.doc or "").split("\n")
        if len(doclines) == 1:
            docstring = f'"""{doclines[0]}"""'
        else:
            docstring = '"""\n' + "\n".join(doclines) + '\n"""'
        docstring = textwrap.indent(docstring, "    ")
        return f"{self.name}{self.signature}:\n{docstring}"

    def __eq__(self, other: Any) -> bool:
        if not isinstance(other, APIDef):
            return NotImplemented

        if self.name in ("__init__", "__iter__", "__len__"):
            # pyo3 has a special way to handle these methods that makes it impossible to match everything.
            # TODO(#7779): Remove this special case once we have a better way to handle these methods
            return self.name == other.name and self.signature == other.signature
        elif self.name in ("__getitem__"):
            # TODO(#7779): It's somehow even worse for these.
            return self.name == other.name
        elif self.internal_object:
            # We don't care about docstrings for internal objects
            return self.name == other.name and self.signature == other.signature
        elif self.getset:
            # Getter/setter methods are complicated. We don't enforce the signatures match.
            return self.name == other.name and self.doc == other.doc
        else:
            return self.name == other.name and self.signature == other.signature and self.doc == other.doc


TotalSignature = dict[str, APIDef | dict[str, APIDef]]


def parse_function_signature(node: Any) -> APIDef:
    """Convert a parso function definition node into a Python inspect.Signature object."""
    params = []

    found_star = False

    for param in node.children[2].children:
        if param.type == "operator":
            if param.value == "*":
                found_star = True
            continue
        param_name = param.name.value

        try:
            default = Parameter.empty

            if param.default:
                code = param.default.get_code()
                try:
                    default = ast.literal_eval(code)
                except Exception as e:
                    # TODO(emilk): When we update to Python 3.11, use e.add_note
                    raise ValueError(f"code='{code}', {e}") from e

            # Determine kind of parameter (positional, keyword, etc.)
            if param.star_count == 1:
                kind: Any = Parameter.VAR_POSITIONAL  # *args
                found_star = True
            elif param.star_count == 2:
                kind = Parameter.VAR_KEYWORD  # **kwargs
            elif param_name == "self":
                kind = Parameter.POSITIONAL_ONLY
            elif found_star:
                kind = Parameter.KEYWORD_ONLY
            else:
                kind = Parameter.POSITIONAL_OR_KEYWORD

            params.append(Parameter(name=param_name, kind=kind, default=default))
        except Exception as e:
            # TODO(emilk): When we update to Python 3.11, use e.add_note
            raise ValueError(f"param_name='{param_name}', {e}") from e

    doc = None
    for child in node.children:
        if child.type == "suite":
            first_child = child.children[1]
            if first_child.type == "simple_stmt" and first_child.children[0].type == "string":
                doc = first_child.children[0].value.replace('"""', "")

    sig = Signature(parameters=params)
    return APIDef(node.name.value, sig, internal_object=False, doc=doc)


def load_stub_signatures(pyi_file: Path) -> TotalSignature:
    """Use parso to parse the .pyi file and convert function and class signatures into inspect.Signature objects."""
    pyi_code = Path(pyi_file).read_text(encoding="utf-8")
    tree = parso.parse(pyi_code)

    signatures: TotalSignature = {}

    for node in tree.children:
        if node.type == "funcdef":
            func_name = node.name.value
            func_signature = parse_function_signature(node)
            signatures[func_name] = func_signature

        elif node.type == "classdef":
            class_name = node.name.value

            try:
                class_def = {}

                doc = None
                for child in node.children:
                    if child.type == "suite":
                        first_child = child.children[1]
                        if first_child.type == "simple_stmt" and first_child.children[0].type == "string":
                            doc = first_child.children[0].value.replace('"""', "")
                if doc is not None:
                    class_def["__doc__"] = doc

                # Extract methods within the class
                for class_node in node.iter_funcdefs():
                    method_name = class_node.name.value

                    try:
                        method_signature = parse_function_signature(class_node)
                    except Exception as e:
                        # TODO(emilk): When we update to Python 3.11, use e.add_note
                        raise ValueError(f"method_name='{method_name}', {e}") from e

                    class_def[method_name] = method_signature

                signatures[class_name] = class_def
            except Exception as e:
                # TODO(emilk): When we update to Python 3.11, use e.add_note
                raise ValueError(f"class_name='{class_name}', {e}") from e

    return signatures


def load_runtime_signatures(module_name: str) -> TotalSignature:
    """Use inspect to extract runtime signatures for both functions and classes."""
    module = importlib.import_module(module_name)

    signatures: TotalSignature = {}

    # Get top-level functions and classes
    for name, obj in inspect.getmembers(module):
        if inspect.isfunction(obj):
            api_def = APIDef(name, inspect.signature(obj), False, obj.__doc__)
            signatures[name] = api_def
        elif inspect.isbuiltin(obj):
            api_def = APIDef(name, inspect.signature(obj), False, obj.__doc__)
            signatures[name] = api_def
        elif inspect.isclass(obj):
            class_def = {}
            is_internal_class = name.endswith("Internal")

            # Get methods within the class
            for method_name, method_obj in inspect.getmembers(obj):
                # Need special handling for __init__ methods because pyo3 doesn't expose them as functions
                # Instead we use the __text_signature__ attribute from the class
                if method_name == "__init__" and obj.__text_signature__ is not None:
                    sig = "def __init__" + obj.__text_signature__ + ": ..."  # NOLINT
                    parsed = parso.parse(sig).children[0]
                    class_def[method_name] = parse_function_signature(parsed)
                    continue
                try:
                    api_def = APIDef(method_name, inspect.signature(method_obj), is_internal_class, method_obj.__doc__)
                    class_def[method_name] = api_def
                except Exception:
                    pass
            # Get property getters
            for method_name, method_obj in inspect.getmembers(
                obj,
                lambda o: o.__class__.__name__ == "getset_descriptor",
            ):
                api_def = APIDef(
                    method_name,
                    Signature(parameters=[Parameter("self", Parameter.POSITIONAL_ONLY)]),
                    is_internal_class,
                    method_obj.__doc__,
                    getset=True,
                )
                class_def[method_name] = api_def
            signatures[name] = class_def

    return signatures


def compare_signatures(stub_signatures: TotalSignature, runtime_signatures: TotalSignature) -> int:
    """Compare stub signatures with runtime signatures."""

    result = 0

    for name, stub_signature in stub_signatures.items():
        if isinstance(stub_signature, dict):
            if name in runtime_signatures:
                runtime_class_signature = runtime_signatures.get(name)
                is_internal_class = name.endswith("Internal")

                if not isinstance(runtime_class_signature, dict):
                    print()
                    print(f"{name} signature mismatch:")
                    print("Stub expected class, but runtime provided function.")
                    continue
                for method_name, stub_method_signature in stub_signature.items():
                    if isinstance(stub_method_signature, str):
                        continue
                    if stub_method_signature.doc is None and not is_internal_class:
                        print()
                        print(f"{name}.{method_name} missing docstring")
                        result += 1
                    runtime_method_signature = runtime_class_signature.get(method_name)
                    if runtime_method_signature != stub_method_signature:
                        print()
                        print(f"{name}.{method_name}(…) signature mismatch:")
                        print_colored_diff(str(runtime_method_signature), str(stub_method_signature))
                        result += 1

            else:
                docstr = stub_signature.get("__doc__", "")
                if isinstance(docstr, str) and "Required-feature:" in docstr:
                    print(f"Skipping class {name} since it requires features not enabled in default runtime")
                else:
                    print(f"Class {name} not found in runtime")
                    result += 1
        else:
            if stub_signature.doc is None:
                print()
                print(f"{name} missing docstring")
                result += 1
            if name in runtime_signatures:
                # Handle top-level functions
                runtime_signature = runtime_signatures.get(name)
                if runtime_signature != stub_signature:
                    print()
                    print(f"{name}(…) signature mismatch:")
                    print_colored_diff(str(runtime_signature), str(stub_signature))
                    result += 1
            else:
                print()
                if stub_signature.doc is not None and "Required-feature:" in stub_signature.doc:
                    print(f"Skipping Function {name} since it requires features not enabled in default runtime")
                else:
                    print(f"Function {name} not found in runtime")
                    result += 1

    if result == 0:
        print("All stub signatures match!")

    return result


def main() -> int:
    # load the stub file
    path_to_stub = Path(__file__).parent.parent.parent / "rerun_py" / "rerun_bindings" / "rerun_bindings.pyi"
    stub_signatures = load_stub_signatures(path_to_stub)

    # load the runtime signatures
    runtime_signatures = load_runtime_signatures("rerun_bindings")

    sys.exit(compare_signatures(stub_signatures, runtime_signatures))


if __name__ == "__main__":
    main()
