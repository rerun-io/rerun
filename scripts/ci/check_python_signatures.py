"""Compares the signatures in `rerun_bindings.pyi` with the actual runtime signatures in `rerun_bindings.so`."""

from __future__ import annotations

import ast
import importlib
import inspect
import sys
from inspect import Parameter, Signature
from pathlib import Path
from typing import Any

import parso

TotalSignature = dict[str, Signature | dict[str, Signature]]


def parse_function_signature(node: Any) -> Signature:
    """Convert a parso function definition node into a Python inspect.Signature object."""
    params = []

    found_star = False

    for param in node.children[2].children:
        if param.type == "operator":
            if param.value == "*":
                found_star = True
            continue
        param_name = param.name.value
        default = Parameter.empty

        if param.default:
            default = ast.literal_eval(param.default.get_code())

        # Determine kind of parameter (positional, keyword, etc.)
        if param.star_count == 1:
            kind: Any = Parameter.VAR_POSITIONAL  # *args
            found_star = True
        elif param.star_count == 2:
            kind = Parameter.VAR_KEYWORD  # **kwargs
        else:
            if param_name == "self":
                kind = Parameter.POSITIONAL_ONLY
            elif found_star:
                kind = Parameter.KEYWORD_ONLY
            else:
                kind = Parameter.POSITIONAL_OR_KEYWORD

        params.append(Parameter(name=param_name, kind=kind, default=default))

    return Signature(parameters=params)


def load_stub_signatures(pyi_file: Path) -> TotalSignature:
    """Use parso to parse the .pyi file and convert function and class signatures into inspect.Signature objects."""
    pyi_code = Path(pyi_file).read_text()
    tree = parso.parse(pyi_code)

    signatures: TotalSignature = {}

    for node in tree.children:
        if node.type == "funcdef":
            func_name = node.name.value
            func_signature = parse_function_signature(node)
            signatures[func_name] = func_signature

        elif node.type == "classdef":
            class_name = node.name.value
            # Extract methods within the class
            class_def = {}
            for class_node in node.iter_funcdefs():
                method_name = class_node.name.value

                method_signature = parse_function_signature(class_node)

                class_def[method_name] = method_signature

            signatures[class_name] = class_def

    return signatures


def load_runtime_signatures(module_name: str) -> TotalSignature:
    """Use inspect to extract runtime signatures for both functions and classes."""
    module = importlib.import_module(module_name)

    signatures: TotalSignature = {}

    # Get top-level functions and classes
    for name, obj in inspect.getmembers(module):
        if inspect.isfunction(obj):
            signatures[name] = inspect.signature(obj)
        elif inspect.isbuiltin(obj):
            signatures[name] = inspect.signature(obj)
        elif inspect.isclass(obj):
            class_def = {}
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
                    class_def[method_name] = inspect.signature(method_obj)
                except Exception:
                    pass
            signatures[name] = class_def

    return signatures


def compare_signatures(stub_signatures: TotalSignature, runtime_signatures: TotalSignature) -> int:
    """Compare stub signatures with runtime signatures."""

    result = 0

    for name, stub_signature in stub_signatures.items():
        if isinstance(stub_signature, dict):
            if name in runtime_signatures:
                runtime_class_signature = runtime_signatures.get(name)
                if not isinstance(runtime_class_signature, dict):
                    print(f"{name} signature mismatch:")
                    print(f"  Stub: {stub_signature}")
                    print(f"  Runtime: {runtime_class_signature}")
                    result += 1
                    continue
                for method_name, stub_method_signature in stub_signature.items():
                    runtime_method_signature = runtime_class_signature.get(method_name)
                    if runtime_method_signature != stub_method_signature:
                        print(f"{name}.{method_name}(…) signature mismatch:")
                        print(f"    Stub: {stub_method_signature}")
                        print(f"    Runtime: {runtime_method_signature}")
                        result += 1

            else:
                print(f"Class {name} not found in runtime")
                result += 1
        else:
            if name in runtime_signatures:
                # Handle top-level functions
                runtime_signature = runtime_signatures.get(name)
                if runtime_signature != stub_signature:
                    print(f"{name}(…) signature mismatch:")
                    print(f"  Stub: {stub_signature}")
                    print(f"  Runtime: {runtime_signature}")
                    result += 1
            else:
                print(f"Function {name} not found in runtime")
                result += 1

    if result == 0:
        print("All stub signatures match!")

    return result


def main() -> int:
    # Assuming your module is called `my_module` and your stub file is `my_module.pyi`
    # def test_validate_bindings():
    path_to_stub = Path(__file__).parent / ".." / ".." / "rerun_py" / "rerun_bindings" / "rerun_bindings.pyi"
    stub_signatures = load_stub_signatures(path_to_stub)

    runtime_signatures = load_runtime_signatures("rerun_bindings")

    sys.exit(compare_signatures(stub_signatures, runtime_signatures))


if __name__ == "__main__":
    main()
