"""Visual step grouping for recompose tasks.

This module provides the `@step` decorator and `step()` context manager
for organizing output within tasks.

- Locally: Output is grouped in a nested tree-view
- In GHA: Uses `::group::` / `::endgroup::` markers (flat, no nesting)
"""

from __future__ import annotations

import functools
import os
from collections.abc import Callable, Generator
from contextlib import contextmanager
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, ParamSpec, TypeVar, overload

if TYPE_CHECKING:
    pass

P = ParamSpec("P")
T = TypeVar("T")


# Environment variable to detect GHA
ENV_GITHUB_ACTIONS = "GITHUB_ACTIONS"


def _is_gha() -> bool:
    """Check if we're running in GitHub Actions."""
    return os.environ.get(ENV_GITHUB_ACTIONS) == "true"


@dataclass
class StepContext:
    """Context for tracking nested steps."""

    name: str
    depth: int = 0
    parent: StepContext | None = None


# Stack of active step contexts (for nesting)
_step_stack: list[StepContext] = []


def _get_current_depth() -> int:
    """Get the current step nesting depth."""
    return len(_step_stack)


def _push_step(name: str) -> StepContext:
    """Push a new step onto the stack."""
    parent = _step_stack[-1] if _step_stack else None
    ctx = StepContext(name=name, depth=len(_step_stack), parent=parent)
    _step_stack.append(ctx)
    return ctx


def _pop_step() -> StepContext | None:
    """Pop the current step from the stack."""
    if _step_stack:
        return _step_stack.pop()
    return None


class StepOutputWrapper:
    """
    Wrapper for stdout that adds step indentation for local output.

    In tree mode, output is indented based on step depth.
    In GHA mode, no transformation is needed (groups are flat).
    """

    def __init__(self, wrapped: Any, indent: str = "  "):
        self._wrapped = wrapped
        self._indent = indent
        self._at_line_start = True

    def write(self, s: str) -> int:
        if not s:
            return 0

        if _is_gha():
            # In GHA, just pass through - groups are flat
            self._wrapped.write(s)
            return len(s)

        # Local mode: add indentation for nested steps
        depth = _get_current_depth()
        if depth == 0:
            self._wrapped.write(s)
            return len(s)

        result = []
        indent_prefix = self._indent * depth
        for char in s:
            if self._at_line_start and char != "\n":
                result.append(indent_prefix)
                self._at_line_start = False
            result.append(char)
            if char == "\n":
                self._at_line_start = True

        output = "".join(result)
        self._wrapped.write(output)
        return len(s)

    def flush(self) -> None:
        self._wrapped.flush()

    def fileno(self) -> int:
        return self._wrapped.fileno()

    @property
    def encoding(self) -> str:
        return getattr(self._wrapped, "encoding", "utf-8")

    def isatty(self) -> bool:
        return self._wrapped.isatty()


@contextmanager
def step(name: str) -> Generator[None, None, None]:
    """
    Context manager for visual output grouping.

    Groups output within a task for better organization.

    - Locally: Creates a nested tree-view with indentation
    - In GHA: Uses `::group::` / `::endgroup::` markers

    Example:
        @task
        def build_and_test() -> Result[None]:
            with recompose.step("Compile"):
                run("cargo", "build", "--release")

            with recompose.step("Run tests"):
                run("cargo", "test")

            return Ok(None)

    Local output:
        build_and_test
          Compile
            cargo build --release
          Run tests
            cargo test
        OK

    GHA output:
        ::group::Compile
        cargo build --release
        ::endgroup::
        ::group::Run tests
        cargo test
        ::endgroup::

    """
    ctx = _push_step(name)

    if _is_gha():
        # GHA mode: use group markers
        print(f"::group::{name}", flush=True)
    else:
        # Local mode: print step header with indentation
        depth = ctx.depth
        indent = "  " * depth
        print(f"{indent}[{name}]", flush=True)

    try:
        yield
    finally:
        _pop_step()
        if _is_gha():
            print("::endgroup::", flush=True)


@overload
def step_decorator(fn: Callable[P, T]) -> Callable[P, T]: ...


@overload
def step_decorator(name: str) -> Callable[[Callable[P, T]], Callable[P, T]]: ...


def step_decorator(
    fn_or_name: Callable[P, T] | str | None = None,
) -> Callable[P, T] | Callable[[Callable[P, T]], Callable[P, T]]:
    """
    Decorator form of step() for helper functions.

    Can be used with or without a name:

        @step_decorator
        def compile_code():
            run("cargo", "build")

        @step_decorator("Custom Name")
        def compile_code():
            run("cargo", "build")

    The output from the decorated function will be grouped
    under the step name.
    """

    def decorator(fn: Callable[P, T], name: str | None = None) -> Callable[P, T]:
        step_name = name or fn.__name__

        @functools.wraps(fn)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
            with step(step_name):
                return fn(*args, **kwargs)

        return wrapper

    # Handle @step_decorator without arguments
    if callable(fn_or_name):
        return decorator(fn_or_name)

    # Handle @step_decorator("name")
    def partial_decorator(fn: Callable[P, T]) -> Callable[P, T]:
        return decorator(fn, fn_or_name)

    return partial_decorator
