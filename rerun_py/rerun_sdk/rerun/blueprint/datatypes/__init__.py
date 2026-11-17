"""
Deprecated alias for [`rerun.blueprint.encodings`][].

`datatypes` was renamed to `encodings` in 0.37, because it clashed with the Arrow `DataType`.
"""

from __future__ import annotations

import warnings
from typing import Any

from .. import encodings

__all__ = encodings.__all__

warnings.warn(
    "`rerun.blueprint.datatypes` is deprecated since 0.37.0. Use `rerun.blueprint.encodings` instead.",
    DeprecationWarning,
    stacklevel=2,
)


# Type checkers give every forwarded name this return type, so it must be `Any` (PEP 562).
def __getattr__(name: str) -> Any:  # noqa: ANN401
    """Forward every lookup to [`rerun.blueprint.encodings`][] (see PEP 562)."""
    return getattr(encodings, name)
