"""Result type for recompose tasks."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Generic, Literal, TypeVar

from pydantic import BaseModel, PrivateAttr

if TYPE_CHECKING:
    from .context import ArtifactInfo

T = TypeVar("T")


class Result(BaseModel, Generic[T]):
    """
    Result of a task execution.

    Use Ok(value) or Err(message) to construct results.
    """

    status: Literal["success", "failure"] = "success"
    error: str | None = None
    traceback: str | None = None
    _value: T | None = PrivateAttr(default=None)
    _outputs: dict[str, str] = PrivateAttr(default_factory=dict)
    _artifacts: dict[str, ArtifactInfo] = PrivateAttr(default_factory=dict)

    model_config = {"frozen": True}  # Make results immutable

    @property
    def ok(self) -> bool:
        """True if the task succeeded."""
        return self.status == "success"

    @property
    def failed(self) -> bool:
        """True if the task failed."""
        return self.status == "failure"

    @property
    def outputs(self) -> dict[str, str]:
        """
        Get task outputs set via set_output().

        Returns an empty dict if no outputs were set.
        """
        return self._outputs

    @property
    def artifacts(self) -> dict[str, ArtifactInfo]:
        """
        Get task artifacts saved via save_artifact().

        Returns an empty dict if no artifacts were saved.
        """
        return self._artifacts

    def value(self) -> T:
        """
        Get the result value.

        Returns the value if the result is successful (including None for Result[None]).
        Raises RuntimeError if the result is a failure.
        """
        if self.failed:
            raise RuntimeError(f"Attempted to get value from a failed result: {self.error}")
        return self._value  # type: ignore[return-value]

    def value_or(self, default: T) -> T:
        """Get the value, or return a default if the result is a failure or has no value."""
        if self.ok and self._value is not None:
            return self._value
        return default


def Ok(value: T) -> Result[T]:
    """Create a successful result with the given value."""
    result = Result[T](status="success")
    object.__setattr__(result, "_value", value)
    return result


def Err(error: str, *, traceback: str | None = None) -> Result[Any]:
    """Create a failed result with an error message.

    Returns Result[Any] so it can be returned from any function
    expecting Result[T] - the value is None for errors anyway.
    """
    return Result(status="failure", error=error, traceback=traceback)
