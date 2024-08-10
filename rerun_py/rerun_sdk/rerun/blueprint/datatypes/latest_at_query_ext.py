from __future__ import annotations

from ... import datatypes


class LatestAtQueryExt:
    """Extension for [LatestAtQuery][rerun.blueprint.datatypes.LatestAtQuery]."""

    # This override is required because otherwise the codegen uses `TimeInt(x)`, which is not valid with the custom
    # `TimeInt.__init__` override.

    @staticmethod
    def time__field_converter_override(x: datatypes.TimeIntLike) -> datatypes.TimeInt:
        if isinstance(x, datatypes.TimeInt):
            return x
        else:
            return datatypes.TimeInt(seq=x)
