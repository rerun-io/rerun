"""Data types for RRD to LeRobot conversion."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ImageSpec:
    """Specification for a video stream in the dataset."""

    key: str
    path: str


@dataclass(frozen=True)
class ColumnSpec:
    """Column names for action, state, and task data."""

    action: str | None
    state: str | None
    task: str | None
