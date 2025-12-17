"""
Expression types for conditional flow execution.

These types capture conditions that can be evaluated at runtime.
They're used with `run_if()` to enable conditional task execution
that works both locally and in GitHub Actions.

The key insight is that we don't need to map expressions to GHA syntax.
Instead, we serialize the expression and run a condition-check task
that evaluates it and outputs a boolean. GHA then gates subsequent
steps on that boolean output.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass


class Expr(ABC):
    """Base class for condition expressions."""

    @abstractmethod
    def serialize(self) -> dict[str, Any]:
        """Serialize to a dict for passing to condition-check task."""
        ...

    @abstractmethod
    def evaluate(self, context: dict[str, Any]) -> Any:
        """
        Evaluate the expression given a context.

        Args:
            context: Dict with 'inputs' (flow params) and 'outputs' (task results)

        """
        ...

    def __and__(self, other: Expr | bool) -> BinaryExpr:
        """Logical AND."""
        return BinaryExpr(self, "and", _to_expr(other))

    def __rand__(self, other: Expr | bool) -> BinaryExpr:
        """Logical AND (reversed)."""
        return BinaryExpr(_to_expr(other), "and", self)

    def __or__(self, other: Expr | bool) -> BinaryExpr:
        """Logical OR."""
        return BinaryExpr(self, "or", _to_expr(other))

    def __ror__(self, other: Expr | bool) -> BinaryExpr:
        """Logical OR (reversed)."""
        return BinaryExpr(_to_expr(other), "or", self)

    def __invert__(self) -> UnaryExpr:
        """Logical NOT (~expr)."""
        return UnaryExpr("not", self)

    def __eq__(self, other: object) -> BinaryExpr:  # type: ignore[override]
        """Equality comparison."""
        return BinaryExpr(self, "==", _to_expr(other))

    def __ne__(self, other: object) -> BinaryExpr:  # type: ignore[override]
        """Inequality comparison."""
        return BinaryExpr(self, "!=", _to_expr(other))

    def __bool__(self) -> bool:
        """Raise error - expressions can't be used in Python control flow."""
        raise TypeError(
            "Condition expressions cannot be used in Python control flow.\n\n"
            "Use 'with recompose.run_if(expr):' instead of 'if expr:'.\n"
            "The run_if context manager creates a conditional block that\n"
            "works both locally and in GitHub Actions."
        )


@dataclass
class LiteralExpr(Expr):
    """A literal value."""

    value: Any

    def serialize(self) -> dict[str, Any]:
        return {"type": "literal", "value": self.value}

    def evaluate(self, context: dict[str, Any]) -> Any:
        return self.value

    def __repr__(self) -> str:
        return f"Literal({self.value!r})"


@dataclass
class InputExpr(Expr):
    """Reference to a flow input parameter."""

    name: str

    def serialize(self) -> dict[str, Any]:
        return {"type": "input", "name": self.name}

    def evaluate(self, context: dict[str, Any]) -> Any:
        inputs = context.get("inputs", {})
        if self.name not in inputs:
            raise KeyError(f"Input '{self.name}' not found in context")
        return inputs[self.name]

    def __repr__(self) -> str:
        return f"Input({self.name})"


@dataclass
class OutputExpr(Expr):
    """Reference to a task's output value."""

    step_name: str

    def serialize(self) -> dict[str, Any]:
        return {"type": "output", "step": self.step_name}

    def evaluate(self, context: dict[str, Any]) -> Any:
        outputs = context.get("outputs", {})
        if self.step_name not in outputs:
            raise KeyError(f"Output for step '{self.step_name}' not found in context")
        return outputs[self.step_name]

    def __repr__(self) -> str:
        return f"Output({self.step_name})"


@dataclass
class BinaryExpr(Expr):
    """Binary operation (comparison or logical)."""

    left: Expr
    op: str  # "==", "!=", "and", "or"
    right: Expr

    def serialize(self) -> dict[str, Any]:
        return {
            "type": "binary",
            "op": self.op,
            "left": self.left.serialize(),
            "right": self.right.serialize(),
        }

    def evaluate(self, context: dict[str, Any]) -> Any:
        left_val = self.left.evaluate(context)
        right_val = self.right.evaluate(context)

        if self.op == "==":
            return left_val == right_val
        elif self.op == "!=":
            return left_val != right_val
        elif self.op == "and":
            return left_val and right_val
        elif self.op == "or":
            return left_val or right_val
        else:
            raise ValueError(f"Unknown operator: {self.op}")

    def __repr__(self) -> str:
        return f"({self.left!r} {self.op} {self.right!r})"


@dataclass
class UnaryExpr(Expr):
    """Unary operation (logical not)."""

    op: str  # "not"
    operand: Expr

    def serialize(self) -> dict[str, Any]:
        return {
            "type": "unary",
            "op": self.op,
            "operand": self.operand.serialize(),
        }

    def evaluate(self, context: dict[str, Any]) -> Any:
        val = self.operand.evaluate(context)

        if self.op == "not":
            return not val
        else:
            raise ValueError(f"Unknown operator: {self.op}")

    def __repr__(self) -> str:
        return f"({self.op} {self.operand!r})"


def _to_expr(value: Any) -> Expr:
    """Convert a value to an Expr."""
    if isinstance(value, Expr):
        return value
    return LiteralExpr(value)


def deserialize_expr(data: dict[str, Any]) -> Expr:
    """Deserialize an expression from a dict."""
    expr_type = data.get("type")

    if expr_type == "literal":
        return LiteralExpr(data["value"])
    elif expr_type == "input":
        return InputExpr(data["name"])
    elif expr_type == "output":
        return OutputExpr(data["step"])
    elif expr_type == "binary":
        return BinaryExpr(
            left=deserialize_expr(data["left"]),
            op=data["op"],
            right=deserialize_expr(data["right"]),
        )
    elif expr_type == "unary":
        return UnaryExpr(
            op=data["op"],
            operand=deserialize_expr(data["operand"]),
        )
    else:
        raise ValueError(f"Unknown expression type: {expr_type}")


def format_expr(data: dict[str, Any]) -> str:
    """Format a serialized condition expression for display."""
    expr_type = data.get("type", "")
    if expr_type == "input":
        return str(data.get("name", "?"))
    elif expr_type == "literal":
        return str(data.get("value", "?"))
    elif expr_type == "output":
        return f"output({data.get('step', '?')})"
    elif expr_type == "binary":
        left = format_expr(data.get("left", {}))
        op = data.get("op", "?")
        right = format_expr(data.get("right", {}))
        return f"{left} {op} {right}"
    elif expr_type == "unary":
        op = data.get("op", "?")
        operand = format_expr(data.get("operand", {}))
        return f"{op} {operand}"
    return "?"
