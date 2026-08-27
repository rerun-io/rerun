"""
Deprecated alias for [`rerun.encodings`][].

`datatypes` was renamed to `encodings` in 0.37, because it clashed with the Arrow `DataType`.
"""

from __future__ import annotations

import warnings
from typing import Any

from .. import encodings

__all__ = encodings.__all__

warnings.warn(
    "`rerun.datatypes` is deprecated since 0.37.0. Use `rerun.encodings` instead.",
    DeprecationWarning,
    stacklevel=2,
)


def __getattr__(name: str) -> Any:
    """Forward every lookup to [`rerun.encodings`][] (see PEP 562)."""
    return getattr(encodings, name)
