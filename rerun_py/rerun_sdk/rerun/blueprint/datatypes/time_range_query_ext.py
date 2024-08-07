from __future__ import annotations

from ... import datatypes


class TimeRangeQueryExt:
    """Extension for [TimeRangeQuery][rerun.blueprint.datatypes.TimeRangeQuery]."""

    # These overrides are required because otherwise the codegen uses `TimeInt(x)`, which is not valid with the custom
    # `TimeInt.__init__` override.

    @staticmethod
    def start__field_converter_override(x: datatypes.TimeIntLike) -> datatypes.TimeInt:
        if isinstance(x, datatypes.TimeInt):
            return x
        else:
            return datatypes.TimeInt(seq=x)

    @staticmethod
    def end__field_converter_override(x: datatypes.TimeIntLike) -> datatypes.TimeInt:
        if isinstance(x, datatypes.TimeInt):
            return x
        else:
            return datatypes.TimeInt(seq=x)
