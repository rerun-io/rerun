from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from . import VisualizerOverridesArrayLike


class VisualizerOverridesExt:
    """Extension for [VisualizerOverrides][rerun.blueprint.components.VisualizerOverrides]."""

    @staticmethod
    def visualizers__field_converter_override(value: str | list[str]) -> list[str]:
        if isinstance(value, str):
            return [value]
        return value

    @staticmethod
    def native_to_pa_array_override(data: VisualizerOverridesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import VisualizerOverrides

        if isinstance(data, VisualizerOverrides):
            array = [data.visualizers]
        elif isinstance(data, str):
            array = [[data]]
        elif isinstance(data, Sequence):
            data = list(data)
            if len(data) == 0:
                array = []
            elif isinstance(data[0], VisualizerOverrides):
                array = [datum.visualizers for datum in data]  # type: ignore[union-attr]
            else:
                array = [[str(datum) for datum in data]]
        else:
            array = [[str(data)]]

        return pa.array(array, type=data_type)
