"""Workspace management for subprocess-isolated flow execution.

A workspace is a directory that stores:
- _params.json: Flow parameters and metadata
- {step_name}.json: Result from each step

This enables subprocess isolation where each step runs independently
and communicates through files.
"""

from __future__ import annotations

import importlib
import json
import os
from abc import ABC, abstractmethod
from dataclasses import asdict, dataclass, is_dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, TypeVar

from pydantic import TypeAdapter

from .result import Err, Ok, Result

T = TypeVar("T")


class Serializer(ABC):
    """Base class for type serializers.

    Implement this to add serialization support for custom types.
    """

    @staticmethod
    @abstractmethod
    def serialize(value: Any) -> Any:
        """Convert value to JSON-serializable form."""
        ...

    @staticmethod
    @abstractmethod
    def deserialize(data: Any) -> Any:
        """Reconstruct value from serialized form."""
        ...


class PathSerializer(Serializer):
    """Serializer for pathlib.Path objects."""

    @staticmethod
    def serialize(value: Path) -> str:
        return str(value)

    @staticmethod
    def deserialize(data: str) -> Path:
        return Path(data)


class DatetimeSerializer(Serializer):
    """Serializer for datetime objects."""

    @staticmethod
    def serialize(value: datetime) -> str:
        return value.isoformat()

    @staticmethod
    def deserialize(data: str) -> datetime:
        return datetime.fromisoformat(data)


# Registry mapping types to their serializers
_serializer_registry: dict[type, type[Serializer]] = {
    Path: PathSerializer,
    datetime: DatetimeSerializer,
}

# Type registry for resolving type keys back to classes
_type_registry: dict[str, type] = {}

# TypeAdapter cache to avoid repeated construction
_adapter_cache: dict[type, TypeAdapter[Any]] = {}


def register_serializer(typ: type, serializer: type[Serializer]) -> None:
    """Register a custom serializer for a type.

    Args:
        typ: The type to register (e.g., PIL.Image.Image)
        serializer: A Serializer subclass that handles serialization

    Example:
        class ImageSerializer(Serializer):
            @staticmethod
            def serialize(img) -> dict:
                return {"mode": img.mode, "data": base64.b64encode(img.tobytes()).decode()}

            @staticmethod
            def deserialize(data: dict) -> Image:
                return Image.frombytes(data["mode"], ...)

        register_serializer(PIL.Image.Image, ImageSerializer)

    """
    _serializer_registry[typ] = serializer


def _get_type_key(cls: type) -> str:
    """Get the type key for a class (module.ClassName)."""
    return f"{cls.__module__}.{cls.__qualname__}"


def _resolve_type(type_key: str) -> type | None:
    """Resolve a type key back to a class."""
    # Check registry first
    if type_key in _type_registry:
        return _type_registry[type_key]

    # Try to import dynamically
    try:
        module_name, class_name = type_key.rsplit(".", 1)
        module = importlib.import_module(module_name)
        cls = getattr(module, class_name, None)
        if cls is not None:
            _type_registry[type_key] = cls
        return cls
    except (ValueError, ImportError, AttributeError):
        return None


def _get_adapter(cls: type) -> TypeAdapter[Any]:
    """Get a cached TypeAdapter for the given class."""
    if cls not in _adapter_cache:
        _adapter_cache[cls] = TypeAdapter(cls)
    return _adapter_cache[cls]


def _is_pydantic_serializable(value: Any) -> bool:
    """Check if a value can be serialized via Pydantic."""
    # Primitives
    if isinstance(value, (str, int, float, bool, type(None))):
        return True
    # Pydantic models
    if hasattr(value, "model_dump"):
        return True
    # Dataclasses
    if is_dataclass(value) and not isinstance(value, type):
        return True
    return False


def _serialize_for_pydantic(value: Any) -> Any:
    """Serialize a value to a form Pydantic can validate on deserialize.

    This converts nested values to JSON-serializable form without type wrappers,
    since Pydantic handles type coercion during validation.
    """
    if value is None:
        return None
    if isinstance(value, (str, int, float, bool)):
        return value

    # Check registry for nested types (including subclasses)
    for registered_type, serializer in _serializer_registry.items():
        if isinstance(value, registered_type):
            return serializer.serialize(value)

    if isinstance(value, (list, tuple)):
        return [_serialize_for_pydantic(v) for v in value]
    if isinstance(value, dict):
        return {k: _serialize_for_pydantic(v) for k, v in value.items()}
    if is_dataclass(value) and not isinstance(value, type):
        return {k: _serialize_for_pydantic(v) for k, v in asdict(value).items()}
    if hasattr(value, "model_dump"):
        return value.model_dump()

    # Should not reach here for properly typed dataclasses/Pydantic models
    raise TypeError(f"Cannot serialize nested value of type {type(value).__name__}")


def serialize_value(value: Any) -> Any:
    """Serialize a value to JSON-serializable form with type information.

    Supported types:
    - Primitives (str, int, float, bool, None)
    - Types with registered serializers (Path, datetime, custom)
    - Pydantic models
    - Dataclasses
    - Lists/dicts containing the above

    Raises:
        TypeError: If the value type is not supported

    """
    if value is None:
        return None

    # Primitives - no wrapper needed
    if isinstance(value, (str, int, float, bool)):
        return value

    # Lists - serialize elements
    if isinstance(value, (list, tuple)):
        return [serialize_value(v) for v in value]

    # Dicts - serialize values (but not if it's our type wrapper)
    if isinstance(value, dict):
        if "__type__" in value:
            return value
        return {k: serialize_value(v) for k, v in value.items()}

    value_type = type(value)

    # Check registry first (including base classes)
    for registered_type, serializer in _serializer_registry.items():
        if isinstance(value, registered_type):
            return {
                "__type__": _get_type_key(registered_type),
                "__value__": serializer.serialize(value),
            }

    # Pydantic models
    if hasattr(value, "model_dump"):
        return {
            "__type__": _get_type_key(value_type),
            "__value__": value.model_dump(),
        }

    # Dataclasses - serialize for Pydantic reconstruction
    if is_dataclass(value) and not isinstance(value, type):
        return {
            "__type__": _get_type_key(value_type),
            "__value__": _serialize_for_pydantic(value),
        }

    # Unsupported type - fail explicitly
    raise TypeError(
        f"Cannot serialize value of type {value_type.__name__}. "
        f"Register a serializer with register_serializer() or use a dataclass/Pydantic model."
    )


def deserialize_value(value: Any) -> Any:
    """Deserialize a JSON value back to Python, restoring types.

    Uses registered serializers for custom types and Pydantic TypeAdapter
    for dataclasses/Pydantic models.
    """
    if value is None:
        return None
    if isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, list):
        return [deserialize_value(v) for v in value]
    if isinstance(value, dict):
        # Check for typed wrapper
        if "__type__" in value:
            type_key = value["__type__"]
            inner_value = value.get("__value__")

            # Try to resolve the type
            cls = _resolve_type(type_key)
            if cls is None:
                # Can't resolve type - return raw value with warning
                return inner_value

            # Check registry first
            if cls in _serializer_registry:
                serializer = _serializer_registry[cls]
                return serializer.deserialize(inner_value)

            # Use Pydantic TypeAdapter for dataclasses/Pydantic models
            try:
                adapter = _get_adapter(cls)
                return adapter.validate_python(inner_value)
            except Exception as e:
                raise TypeError(f"Failed to deserialize {type_key}: {e}") from e

        # Regular dict - deserialize values
        return {k: deserialize_value(v) for k, v in value.items()}

    return value


# Keep old names for backwards compatibility
_serialize_value = serialize_value
_deserialize_value = deserialize_value


@dataclass
class FlowParams:
    """Flow parameters and metadata stored in _params.json."""

    flow_name: str
    params: dict[str, Any]
    steps: list[str]  # Step names in execution order
    created_at: str
    script_path: str  # Path to the script (for subprocess invocation)

    def to_json(self) -> str:
        """Serialize to JSON string."""
        return json.dumps(asdict(self), indent=2)

    @classmethod
    def from_json(cls, data: str) -> FlowParams:
        """Deserialize from JSON string."""
        d = json.loads(data)
        return cls(**d)


def get_default_workspace_root() -> Path:
    """Get the default root directory for workspaces."""
    # Check for environment variable override (useful in CI)
    if env_workspace := os.environ.get("RECOMPOSE_WORKSPACE"):
        return Path(env_workspace)

    # Default to ~/.recompose/runs/
    return Path.home() / ".recompose" / "runs"


def create_workspace(flow_name: str, workspace: Path | None = None) -> Path:
    """
    Create a new workspace directory for a flow run.

    Args:
        flow_name: Name of the flow
        workspace: Explicit workspace path, or None for auto-generated

    Returns:
        Path to the workspace directory

    """
    if workspace is not None:
        workspace.mkdir(parents=True, exist_ok=True)
        return workspace

    # Generate a unique workspace directory
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    root = get_default_workspace_root()
    ws = root / f"{flow_name}_{timestamp}"
    ws.mkdir(parents=True, exist_ok=True)
    return ws


def write_params(workspace: Path, params: FlowParams) -> None:
    """Write flow parameters to _params.json."""
    workspace.mkdir(parents=True, exist_ok=True)
    params_file = workspace / "_params.json"
    params_file.write_text(params.to_json())


def read_params(workspace: Path) -> FlowParams:
    """Read flow parameters from _params.json."""
    params_file = workspace / "_params.json"
    if not params_file.exists():
        raise FileNotFoundError(f"No _params.json found in {workspace}")
    return FlowParams.from_json(params_file.read_text())


def write_step_result(workspace: Path, step_name: str, result: Result[Any]) -> None:
    """
    Write a step's result to {step_name}.json.

    Args:
        workspace: Workspace directory
        step_name: Name of the step (e.g., "01_fetch_source")
        result: The Result to serialize

    """
    result_file = workspace / f"{step_name}.json"
    data = {
        "status": result.status,
        "value": serialize_value(result._value),
        "error": result.error,
        "traceback": result.traceback,
    }
    result_file.write_text(json.dumps(data, indent=2))


def read_step_result(workspace: Path, step_name: str) -> Result[Any]:
    """
    Read a step's result from {step_name}.json.

    Args:
        workspace: Workspace directory
        step_name: Name of the step (e.g., "01_fetch_source")

    Returns:
        The deserialized Result

    """
    result_file = workspace / f"{step_name}.json"
    if not result_file.exists():
        return Err(f"Step result not found: {step_name}")

    data = json.loads(result_file.read_text())

    if data["status"] == "success":
        return Ok(deserialize_value(data["value"]))
    else:
        result: Result[Any] = Err(data.get("error", "Unknown error"), traceback=data.get("traceback"))
        return result


def step_result_exists(workspace: Path, step_name: str) -> bool:
    """Check if a step's result file exists."""
    return (workspace / f"{step_name}.json").exists()


def get_workspace_from_env() -> Path | None:
    """Get workspace path from environment variable if set."""
    if ws := os.environ.get("RECOMPOSE_WORKSPACE"):
        return Path(ws)
    return None
