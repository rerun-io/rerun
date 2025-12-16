"""
Conditional execution support for flows.

Provides the `run_if()` context manager for conditional task execution
that works both locally and in GitHub Actions.

Usage:
    @recompose.flow
    def my_flow(*, full_tests: bool = False) -> None:
        build()

        with recompose.run_if(full_tests):
            full_test()  # Only runs if full_tests is true
"""

from __future__ import annotations

from collections.abc import Generator
from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

from .expr import Expr, LiteralExpr, deserialize_expr
from .result import Ok, Result

if TYPE_CHECKING:
    from .flowgraph import InputPlaceholder


@dataclass
class ConditionalBlock:
    """Represents an active conditional block."""

    condition: Expr
    """The condition expression."""

    block_id: str = field(default_factory=lambda: f"cond_{id(object())}")
    """Unique ID for this conditional block."""


# Context variable tracking the current conditional block (if any)
_current_condition: ContextVar[ConditionalBlock | None] = ContextVar("_current_condition", default=None)


def get_current_condition() -> ConditionalBlock | None:
    """Get the current conditional block, if any."""
    return _current_condition.get()


def _to_expr(value: Any) -> Expr:
    """Convert a value to an Expr."""
    if isinstance(value, Expr):
        return value
    # Check if it's an InputPlaceholder (avoid circular import)
    if hasattr(value, "to_expr") and callable(value.to_expr):
        result: Expr = value.to_expr()
        return result
    return LiteralExpr(value)


@contextmanager
def run_if(
    condition: Expr | InputPlaceholder[bool] | bool,
) -> Generator[None, None, None]:
    """
    Context manager for conditional task execution.

    Tasks created within this block will only execute if the condition is true.
    This works both for local execution and GitHub Actions workflows.

    Args:
        condition: The condition to evaluate. Can be:
            - A flow parameter (InputPlaceholder)
            - An expression (e.g., `param == "value"`)
            - A literal boolean (mostly for testing)

    Example:
        @recompose.flow
        def my_flow(*, debug: bool = False) -> None:
            build()

            with recompose.run_if(debug):
                print_debug_info()

            deploy()

    For GHA:
        - A condition-check task is created that evaluates the condition
        - Tasks in the block get `if: ${{ steps.condition_check.outputs.value == 'true' }}`

    For local execution:
        - The condition is evaluated with actual parameter values
        - Tasks are skipped if the condition is false

    """
    expr = _to_expr(condition)
    block = ConditionalBlock(condition=expr)

    token = _current_condition.set(block)
    try:
        yield
    finally:
        _current_condition.reset(token)


def evaluate_condition(
    condition_data: dict[str, Any],
    inputs: dict[str, Any],
    outputs: dict[str, Any],
) -> Result[bool]:
    """
    Evaluate a serialized condition expression.

    This is called when executing a condition-check step.

    Args:
        condition_data: Serialized expression from Expr.serialize()
        inputs: Flow input parameter values
        outputs: Previous step output values (step_name -> value)

    Returns:
        Result[bool] with the condition evaluation result

    """
    expr = deserialize_expr(condition_data)
    context = {"inputs": inputs, "outputs": outputs}
    result = expr.evaluate(context)
    return Ok(bool(result))
