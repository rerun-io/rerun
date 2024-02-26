from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any, Iterable, Sequence, cast

import pyarrow as pa

from .keypoint_pair_ext import _keypoint_pair_converter

if TYPE_CHECKING:
    from .. import ComponentBatchLike
    from . import (
        AnnotationInfo,
        AnnotationInfoLike,
        ClassDescription,
        ClassDescriptionArrayLike,
        ClassDescriptionLike,
        KeypointPair,
        KeypointPairLike,
    )


def _class_description_converter(
    data: ClassDescriptionLike,
) -> ClassDescription:
    from . import ClassDescription

    if isinstance(data, ClassDescription):
        return data
    else:
        return ClassDescription(info=data)


class ClassDescriptionExt:
    """Extension for [ClassDescription][rerun.datatypes.ClassDescription]."""

    def __init__(
        self: Any,
        *,
        info: AnnotationInfoLike,
        keypoint_annotations: Sequence[AnnotationInfoLike] | None = [],
        keypoint_connections: Sequence[KeypointPairLike] | None = [],
    ) -> None:
        """
        Create a new instance of the ClassDescription datatype.

        Parameters
        ----------
        info:
            The `AnnotationInfo` for the class.
        keypoint_annotations:
            The `AnnotationInfo` for all the keypoints.
        keypoint_connections:
            The connections between keypoints.

        """

        # Always convert None to empty list
        if keypoint_annotations is None:
            keypoint_annotations = []
        if keypoint_connections is None:
            keypoint_connections = []
        self.__attrs_init__(
            info=info, keypoint_annotations=keypoint_annotations, keypoint_connections=keypoint_connections
        )

    # Implement the AsComponents protocol
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..archetypes import AnnotationContext
        from . import ClassDescription

        return AnnotationContext(cast(ClassDescription, self)).as_component_batches()

    @staticmethod
    def info__field_converter_override(
        data: AnnotationInfoLike,
    ) -> AnnotationInfo:
        from . import AnnotationInfo

        if isinstance(data, AnnotationInfo):
            return data
        elif isinstance(data, int):
            return AnnotationInfo(id=data)
        else:
            return AnnotationInfo(*data)

    @staticmethod
    def keypoint_annotations__field_converter_override(
        data: Sequence[AnnotationInfoLike] | None,
    ) -> list[AnnotationInfo] | None:
        if data is None:
            return data

        return [ClassDescriptionExt.info__field_converter_override(item) for item in data]

    @staticmethod
    def keypoint_connections__field_converter_override(
        data: Sequence[KeypointPairLike] | None,
    ) -> list[KeypointPair] | None:
        if data is None:
            return data

        return [_keypoint_pair_converter(item) for item in data]

    @staticmethod
    def native_to_pa_array_override(data: ClassDescriptionArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import AnnotationInfoBatch, ClassDescription, KeypointPairBatch

        if isinstance(data, ClassDescription):
            data = [data]

        descs = [_class_description_converter(item) for item in data]

        infos = [item.info for item in descs]
        annotations = [item.keypoint_annotations for item in descs]
        connections = [item.keypoint_connections for item in descs]

        infos_array = AnnotationInfoBatch(infos).as_arrow_array().storage

        annotation_offsets = list(
            itertools.chain([0], itertools.accumulate(len(ann) if ann else 0 for ann in annotations))
        )
        # TODO(jleibs): Re-enable null support
        # annotation_null_map = pa.array([ann is None for ann in annotations], type=pa.bool_())
        # concat_annotations = list(itertools.chain.from_iterable(ann for ann in annotations if ann is not None))
        concat_annotations = list(itertools.chain.from_iterable(annotations))
        annotation_values_array = AnnotationInfoBatch(concat_annotations).as_arrow_array().storage
        # annotations_array = pa.ListArray.from_arrays(annotation_offsets,
        #                                              annotation_values_array,
        #                                              mask=annotation_null_map)
        annotations_array = pa.ListArray.from_arrays(annotation_offsets, annotation_values_array).cast(
            data_type.field("keypoint_annotations").type
        )

        connections_offsets = list(
            itertools.chain([0], itertools.accumulate(len(con) if con else 0 for con in connections))
        )
        # TODO(jleibs): Re-enable null support
        # connection_null_map = pa.array([con is None for con in connections], type=pa.bool_())
        # concat_connections = list(itertools.chain.from_iterable(con for con in connections if con is not None))
        concat_connections = list(itertools.chain.from_iterable(connections))
        connection_values_array = KeypointPairBatch(concat_connections).as_arrow_array().storage
        # connection_array = pa.ListArray.from_arrays(connections_offsets,
        #                                             connection_values_array,
        #                                             mask=connection_null_map)
        connection_array = pa.ListArray.from_arrays(connections_offsets, connection_values_array).cast(
            data_type.field("keypoint_connections").type
        )

        return pa.StructArray.from_arrays(
            arrays=[infos_array, annotations_array, connection_array],
            fields=[
                data_type.field("info"),
                data_type.field("keypoint_annotations"),
                data_type.field("keypoint_connections"),
            ],
        )
