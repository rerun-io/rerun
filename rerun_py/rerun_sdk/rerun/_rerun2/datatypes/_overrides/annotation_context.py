from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from .. import (
        AnnotationInfo,
        AnnotationInfoArrayLike,
        AnnotationInfoLike,
        ClassDescription,
        ClassDescriptionArrayLike,
        ClassDescriptionLike,
        ClassDescriptionMapElem,
        ClassDescriptionMapElemArrayLike,
        ClassDescriptionMapElemLike,
        KeypointPair,
        KeypointPairArrayLike,
        KeypointPairLike,
    )


################################################################################
# Init overrides
################################################################################


def classdescription_init(
    self: ClassDescription,
    info: AnnotationInfoLike,
    keypoint_annotations: Sequence[AnnotationInfoLike] = [],
    keypoint_connections: Sequence[KeypointPairLike] = [],
) -> None:
    self.__attrs_init__(info=info, keypoint_annotations=keypoint_annotations, keypoint_connections=keypoint_connections)


################################################################################
# Internal converters
################################################################################


def _keypoint_pair_converter(
    data: KeypointPairLike,
) -> KeypointPair:
    from .. import KeypointPair

    if isinstance(data, KeypointPair):
        return data
    else:
        return KeypointPair(*data)


def _class_description_converter(
    data: ClassDescriptionLike,
) -> ClassDescription:
    from .. import ClassDescription

    if isinstance(data, ClassDescription):
        return data
    else:
        return ClassDescription(info=data)


def _class_description_map_elem_converter(
    data: ClassDescriptionMapElemLike,
) -> ClassDescriptionMapElem:
    from .. import ClassDescription, ClassDescriptionMapElem

    if isinstance(data, ClassDescriptionMapElem):
        return data
    elif isinstance(data, ClassDescription):
        return ClassDescriptionMapElem(class_id=data.info.id, class_description=data)
    else:
        desc = ClassDescription(info=data)
        return ClassDescriptionMapElem(class_id=desc.info.id, class_description=desc)


################################################################################
# Field converters
################################################################################


def classdescription_info_converter(
    data: AnnotationInfoLike,
) -> AnnotationInfo:
    from .. import AnnotationInfo

    if isinstance(data, AnnotationInfo):
        return data
    else:
        return AnnotationInfo(*data)


def classdescription_keypoint_annotations_converter(
    data: Sequence[AnnotationInfoLike] | None,
) -> list[AnnotationInfo] | None:
    if data is None:
        return data

    return [classdescription_info_converter(item) for item in data]


def classdescription_keypoint_connections_converter(
    data: Sequence[KeypointPairLike] | None,
) -> list[KeypointPair] | None:
    if data is None:
        return data

    return [_keypoint_pair_converter(item) for item in data]


################################################################################
# Arrow converters
################################################################################


def keypointpair_native_to_pa_array(data: KeypointPairArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import KeypointIdArray, KeypointPair

    if isinstance(data, KeypointPair):
        data = [data]

    keypoints = [_keypoint_pair_converter(item) for item in data]

    keypoint0 = [pair.keypoint0 for pair in keypoints]
    keypoint1 = [pair.keypoint1 for pair in keypoints]

    keypoint0_array = KeypointIdArray.from_similar(keypoint0).storage
    keypoint1_array = KeypointIdArray.from_similar(keypoint1).storage

    return pa.StructArray.from_arrays(
        arrays=[keypoint0_array, keypoint1_array],
        fields=[data_type.field("keypoint0"), data_type.field("keypoint1")],
    )


def annotationinfo_native_to_pa_array(data: AnnotationInfoArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import ColorType, LabelType, AnnotationInfo

    if isinstance(data, AnnotationInfo):
        data = [data]

    annotations = [classdescription_info_converter(item) for item in data]

    ids = [item.id for item in annotations]
    labels = [item.label.value if item.label else None for item in annotations]
    colors = [item.color.rgba if item.color else None for item in annotations]

    id_array = pa.array(ids, type=pa.uint16())

    # Note: we can't use from_similar here because we need to handle optional values
    # fortunately these are fairly simple types
    label_array = pa.array(labels, type=LabelType().storage_type)
    color_array = pa.array(colors, type=ColorType().storage_type)

    return pa.StructArray.from_arrays(
        arrays=[id_array, label_array, color_array],
        fields=[data_type.field("id"), data_type.field("label"), data_type.field("color")],
    )


def classdescription_native_to_pa_array(data: ClassDescriptionArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import AnnotationInfoArray, ClassDescription, KeypointPairArray

    if isinstance(data, ClassDescription):
        data = [data]

    descs = [_class_description_converter(item) for item in data]

    infos = [item.info for item in descs]
    annotations = [item.keypoint_annotations for item in descs]
    connections = [item.keypoint_connections for item in descs]

    infos_array = AnnotationInfoArray.from_similar(infos).storage

    annotation_offsets = list(itertools.chain([0], itertools.accumulate(len(ann) if ann else 0 for ann in annotations)))
    # TODO(jleibs): Re-enable null support
    # annotation_null_map = pa.array([ann is None for ann in annotations], type=pa.bool_())
    # concat_annotations = list(itertools.chain.from_iterable(ann for ann in annotations if ann is not None))
    concat_annotations = list(itertools.chain.from_iterable(annotations))
    annotation_values_array = AnnotationInfoArray.from_similar(concat_annotations).storage
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
    connection_values_array = KeypointPairArray.from_similar(concat_connections).storage
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


def classdescriptionmapelem_native_to_pa_array(
    data: ClassDescriptionMapElemArrayLike, data_type: pa.DataType
) -> pa.Array:
    from .. import ClassIdArray, ClassDescriptionArray, ClassDescriptionMapElem

    if isinstance(data, ClassDescriptionMapElem):
        data = [data]

    map_items = [_class_description_map_elem_converter(item) for item in data]

    ids = [item.class_id for item in map_items]
    class_descriptions = [item.class_description for item in map_items]

    id_array = ClassIdArray.from_similar(ids).storage
    desc_array = ClassDescriptionArray.from_similar(class_descriptions).storage

    return pa.StructArray.from_arrays(
        arrays=[id_array, desc_array],
        fields=[data_type.field("class_id"), data_type.field("class_description")],
    )
