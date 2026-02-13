from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import VisualizerComponentMappingArrayLike


class VisualizerComponentMappingExt:
    """Extension for [VisualizerComponentMapping][rerun.blueprint.datatypes.VisualizerComponentMapping]."""

    @staticmethod
    def native_to_pa_array_override(data: VisualizerComponentMappingArrayLike, data_type: pa.DataType) -> pa.Array:
        from rerun.blueprint.datatypes import ComponentSourceKindBatch
        from rerun.datatypes import Utf8Batch

        from . import VisualizerComponentMapping

        if isinstance(data, VisualizerComponentMapping):
            typed_data = [data]
        else:
            typed_data = list(data)

        # target: non-nullable Utf8 - use Utf8Batch
        target_array = Utf8Batch([x.target for x in typed_data]).as_arrow_array()

        # source_kind: non-nullable enum
        source_kind_array = ComponentSourceKindBatch([x.source_kind for x in typed_data]).as_arrow_array()

        # source_component: nullable string - serialize directly to preserve None
        source_component_array = pa.array(
            [x.source_component for x in typed_data],
            type=pa.utf8(),
        )

        # selector: nullable string - serialize directly to preserve None
        selector_array = pa.array(
            [x.selector for x in typed_data],
            type=pa.utf8(),
        )

        return pa.StructArray.from_arrays(
            [target_array, source_kind_array, source_component_array, selector_array],
            fields=list(data_type),
        )
