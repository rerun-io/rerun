from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.components import AnnotationContext, AnnotationContextLike
from rerun.datatypes import (
    AnnotationInfo,
    AnnotationInfoLike,
    ClassDescription,
    ClassDescriptionArrayLike,
    ClassDescriptionMapElem,
    KeypointPair,
    KeypointPairLike,
    Rgba32,
    Utf8,
)

if TYPE_CHECKING:
    from collections.abc import Sequence

ANNOTATION_INFO_INPUTS: list[AnnotationInfoLike] = [
    AnnotationInfo(1, "label", Rgba32([1, 2, 3])),
    AnnotationInfo(1, color=Rgba32([1, 2, 3])),
    (1, "label"),
    (1, "label", [1, 2, 3]),
]

KEYPOINT_MAP_INPUTS: list[Sequence[AnnotationInfoLike] | None] = [
    None,
    [],
    [
        (1, "label1"),
        (2, "label2"),
    ],
    [
        (1, "label1", [1, 2, 3]),
        (2, "label2", [4, 5, 6]),
    ],
    [
        AnnotationInfo(1, "label1", Rgba32([1, 2, 3])),
        AnnotationInfo(2, "label2", Rgba32([4, 5, 6])),
    ],
]

KEYPOINT_CONNECTIONS_INPUTS: list[Sequence[KeypointPairLike] | None] = [
    None,
    [],
    [
        (1, 2),
    ],
    [
        [1, 2],
    ],
    [
        KeypointPair(1, 2),
    ],
]


def assert_correct_class_description(desc: ClassDescription) -> None:
    assert desc.info.id == 1
    if desc.info.label:
        assert desc.info.label == Utf8("label")
    if desc.info.color:
        assert desc.info.color == Rgba32([1, 2, 3])
    if desc.keypoint_annotations:
        expected_annotations = [
            AnnotationInfo(1, "label1", Rgba32([1, 2, 3])),
            AnnotationInfo(2, "label2", Rgba32([4, 5, 6])),
        ]
        for i, kp in enumerate(desc.keypoint_annotations):
            assert kp.id == expected_annotations[i].id
            if kp.label:
                assert kp.label == expected_annotations[i].label
            if kp.color:
                assert kp.color == expected_annotations[i].color
    if desc.keypoint_connections:
        expected_pairs = [
            KeypointPair(1, 2),
        ]
        if len(desc.keypoint_connections) > 0:
            assert desc.keypoint_connections == expected_pairs


@pytest.mark.parametrize("input", ANNOTATION_INFO_INPUTS)
@pytest.mark.parametrize("keypoint_annotations", KEYPOINT_MAP_INPUTS)
@pytest.mark.parametrize("keypoint_connections", KEYPOINT_CONNECTIONS_INPUTS)
def test_class_description(
    input: AnnotationInfoLike,
    keypoint_annotations: Sequence[AnnotationInfoLike] | None,
    keypoint_connections: Sequence[KeypointPairLike] | None,
) -> None:
    assert_correct_class_description(
        ClassDescription(
            info=input,
            keypoint_annotations=keypoint_annotations,
            keypoint_connections=keypoint_connections,
        ),
    )


ANNOTATION_CONTEXT_INPUTS: list[ClassDescriptionArrayLike] = [
    [
        (1, "label1", [1, 2, 3]),
        (2, "label2", [4, 5, 6]),
    ],
    [
        ClassDescription(info=(1, "label1", [1, 2, 3]), keypoint_annotations=[(3, "kp_label1", [7, 8, 9])]),
        ClassDescription(info=(2, "label2", [4, 5, 6]), keypoint_annotations=[(4, "kp_label2", [10, 11, 12])]),
    ],
    [
        AnnotationInfo(1, "label1"),
        AnnotationInfo(2, color=[4, 5, 6]),
    ],
    [
        ClassDescription(info=(1, "label1"), keypoint_annotations=[(3, "kp_label1")]),
        ClassDescription(info=(2, "label2", [4, 5, 6]), keypoint_annotations=[(4, "kp_label2", [10, 11, 12])]),
    ],
    [
        ClassDescription(info=(1, "label1"), keypoint_connections=[(1, 2)]),
        ClassDescription(info=(2, "label2")),
    ],
]


def assert_correct_annotation_context(ctx: AnnotationContext) -> None:
    assert len(ctx.class_map) == 2
    expected_classes = [
        ClassDescriptionMapElem(
            class_id=1,
            class_description=ClassDescription(
                info=(1, "label1", [1, 2, 3]),
                keypoint_annotations=[(3, "kp_label1", [7, 8, 9])],
            ),
        ),
        ClassDescriptionMapElem(
            class_id=2,
            class_description=ClassDescription(
                info=(2, "label2", [4, 5, 6]),
                keypoint_annotations=[(4, "kp_label2", [10, 11, 12])],
            ),
        ),
    ]
    for i, item in enumerate(ctx.class_map):
        assert item.class_id == expected_classes[i].class_id
        assert item.class_description.info.id == expected_classes[i].class_description.info.id
        if item.class_description.info.label:
            assert item.class_description.info.label == expected_classes[i].class_description.info.label
        if item.class_description.info.color:
            assert item.class_description.info.color == expected_classes[i].class_description.info.color
        if item.class_description.keypoint_annotations:
            for j, kp in enumerate(item.class_description.keypoint_annotations):
                assert kp.id == expected_classes[i].class_description.keypoint_annotations[j].id
                if kp.label:
                    assert kp.label == expected_classes[i].class_description.keypoint_annotations[j].label
                if kp.color:
                    assert kp.color == expected_classes[i].class_description.keypoint_annotations[j].color


@pytest.mark.parametrize("ctx", ANNOTATION_CONTEXT_INPUTS)
def test_annotation_context_component(ctx: Sequence[ClassDescriptionMapElem]) -> None:
    assert_correct_annotation_context(AnnotationContext(ctx))


ANNOTATION_ARCH_INPUTS: Sequence[ClassDescriptionArrayLike] = [
    ClassDescription(info=(1, "label1", [1, 2, 3])),
    *ANNOTATION_CONTEXT_INPUTS,
]


@pytest.mark.parametrize("ctx", ANNOTATION_ARCH_INPUTS)
def test_annotation_context_arch(ctx: AnnotationContextLike) -> None:
    # Verify we can construct the archetype
    rr.AnnotationContext(ctx)

    # TODO(jleibs): Actually verify the serialized arrow data has the right values
