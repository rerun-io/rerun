from __future__ import annotations

from ... import encodings


class FilterByRangeExt:
    """Extension for [FilterByRange][rerun.blueprint.encodings.FilterByRange]."""

    # These overrides are required because otherwise the codegen uses `TimeInt(x)`, which is not valid with the custom
    # `TimeInt.__init__` override.

    @staticmethod
    def start__field_converter_override(x: encodings.TimeIntLike) -> encodings.TimeInt:
        if isinstance(x, encodings.TimeInt):
            return x
        else:
            return encodings.TimeInt(seq=x)

    @staticmethod
    def end__field_converter_override(x: encodings.TimeIntLike) -> encodings.TimeInt:
        if isinstance(x, encodings.TimeInt):
            return x
        else:
            return encodings.TimeInt(seq=x)
