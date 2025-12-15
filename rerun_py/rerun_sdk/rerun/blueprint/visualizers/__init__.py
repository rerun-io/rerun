from __future__ import annotations

from typing import Any

# Re-export all non-experimental visualizer name constants
from .mapping import *  # noqa: F403
from .mapping import experimental as _experimental_base


class experimental(_experimental_base):
    # Override with our own Visualizer base class
    # TODO(RR-3153, RR-3173): Add new APIs that are descriptor aware here.
    class Visualizer:
        def __init__(self, visualizer_type: str, *, overrides: Any = None, mappings: Any = None) -> None:
            self.visualizer_type = visualizer_type
            self.overrides = overrides
            self.mappings = mappings or []

