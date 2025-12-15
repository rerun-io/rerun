"""Workspace management for subprocess-isolated flow execution.

A workspace is a directory that stores:
- _params.json: Flow parameters and metadata
- {step_name}.json: Result from each step

This enables subprocess isolation where each step runs independently
and communicates through files.
"""

from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from .result import Err, Ok, Result


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


def _serialize_value(value: Any) -> Any:
    """Convert a value to JSON-serializable form."""
    if value is None:
        return None
    if isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, (list, tuple)):
        return [_serialize_value(v) for v in value]
    if isinstance(value, dict):
        return {k: _serialize_value(v) for k, v in value.items()}
    # Try to get __dict__ for objects
    if hasattr(value, "__dict__"):
        return _serialize_value(value.__dict__)
    # Fall back to string representation
    return str(value)


def _deserialize_value(value: Any, type_hint: type | None = None) -> Any:
    """Convert a JSON value back to Python, with optional type hint."""
    if value is None:
        return None
    if type_hint is Path or (isinstance(value, str) and type_hint is None):
        # Keep strings as strings by default, caller can convert to Path if needed
        return value
    return value


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
        "value": _serialize_value(result._value),
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
        return Ok(_deserialize_value(data["value"]))
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
