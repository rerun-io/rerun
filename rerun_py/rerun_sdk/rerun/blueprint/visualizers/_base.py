"""Base class for visualizer configuration."""

from __future__ import annotations

from typing import Any


class Visualizer:
    """Base class for visualizer configuration."""

    def __init__(self, visualizer_type: str, *, overrides: Any = None, mappings: Any = None) -> None:
        self.visualizer_type = visualizer_type
        self.overrides = overrides
        self.mappings = mappings or []

        # TODO(RR-3153, RR-3173): Add new APIs that are descriptor aware here.
