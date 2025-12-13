"""Result type for recompose tasks."""

from __future__ import annotations

from typing import Generic, Literal, TypeVar

from pydantic import BaseModel

T = TypeVar("T")


class Result(BaseModel, Generic[T]):
    """
    Result of a task execution.

    Use Ok(value) or Err(message) to construct results.
    """

    value: T | None = None
    status: Literal["success", "failure"] = "success"
    error: str | None = None
    traceback: str | None = None

    model_config = {"frozen": True}  # Make results immutable

    @property
    def ok(self) -> bool:
        """True if the task succeeded."""
        return self.status == "success"

    @property
    def failed(self) -> bool:
        """True if the task failed."""
        return self.status == "failure"

    def unwrap(self) -> T:
        """
        Get the value, raising an error if the result is a failure.

        Raises:
            RuntimeError: If the result is a failure.
        """
        if self.failed:
            raise RuntimeError(f"Attempted to unwrap a failed result: {self.error}")
        if self.value is None:
            raise RuntimeError("Attempted to unwrap a result with no value")
        return self.value

    def unwrap_or(self, default: T) -> T:
        """Get the value, or return a default if the result is a failure."""
        if self.ok and self.value is not None:
            return self.value
        return default


def Ok(value: T) -> Result[T]:
    """Create a successful result with the given value."""
    return Result(value=value, status="success")


def Err(error: str, *, traceback: str | None = None) -> Result[None]:
    """Create a failed result with an error message."""
    return Result(status="failure", error=error, traceback=traceback)
