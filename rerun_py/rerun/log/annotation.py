from dataclasses import dataclass
from typing import Iterable, Optional, Sequence, Tuple, Union

from rerun.log import Color, _normalize_colors

from rerun import bindings

__all__ = [
    "AnnotationInfo",
    "ClassDescription",
    "log_annotation_context",
]


@dataclass
class AnnotationInfo:
    """
    Annotation info annotating a class id or key-point id.

    Color and label will be used to annotate objects/keypoints which reference the id.
    The id refers either to a class or key-point id
    """

    id: int = 0
    label: Optional[str] = None
    color: Optional[Color] = None


AnnotationInfoLike = Union[Tuple[int, str], Tuple[int, str, Color], AnnotationInfo]


def coerce_annotation_info(arg: AnnotationInfoLike) -> AnnotationInfo:
    if type(arg) is AnnotationInfo:
        return arg
    else:
        return AnnotationInfo(*arg)  # type: ignore[misc]


@dataclass
class ClassDescription:
    """
    Metadata about a class type identified by an id.

    Typically a class description contains only a annotation info.
    However, within a class there might be several keypoints, each with its own annotation info.
    Keypoints in turn may be connected to each other by connections (typically used for skeleton edges).
    """

    info: Optional[AnnotationInfoLike] = None
    keypoint_annotations: Optional[Iterable[AnnotationInfoLike]] = None
    keypoint_connections: Optional[Iterable[Union[int, Tuple[int, int]]]] = None


ClassDescriptionLike = Union[AnnotationInfoLike, ClassDescription]


def coerce_class_descriptor_like(arg: ClassDescriptionLike) -> ClassDescription:
    if type(arg) is ClassDescription:
        return arg
    else:
        return ClassDescription(info=arg)  # type: ignore[arg-type]


def log_annotation_context(
    obj_path: str,
    class_descriptions: Union[ClassDescriptionLike, Iterable[ClassDescriptionLike]],
    *,
    timeless: bool = True,
) -> None:
    """
    Log an annotation context made up of a collection of ClassDescriptions.

    Any object needing to access the annotation context will find it by searching the
    path upward. If all objects share the same you can simply log it to the
    root ("/"), or if you want a per-object ClassDescriptions log it to the same path as
    your object.

    Each ClassDescription must include an annotation info with an id, which will be used for matching
    the class and may optionally include a label and color. Colors should
    either be in 0-255 gamma space or in 0-1 linear space. Colors can be RGB or
    RGBA.

    These can either be specified verbosely as:
    ```
    [AnnotationInfo(id=23, label='foo', color=(255, 0, 0)), ...]
    ```

    Or using short-hand tuples.
    ```
    [(23, 'bar'), ...]
    ```

    Unspecified colors will be filled in by the visualizer randomly.
    """
    if not isinstance(class_descriptions, Iterable):
        class_descriptions = [class_descriptions]

    # Coerce tuples into ClassDescription dataclass for convenience
    typed_class_descriptions = (coerce_class_descriptor_like(d) for d in class_descriptions)

    # Convert back to fixed tuple for easy pyo3 conversion
    # This is pretty messy but will likely go away / be refactored with pending data-model changes.
    def info_to_tuple(info: Optional[AnnotationInfoLike]) -> Tuple[int, Optional[str], Optional[Sequence[int]]]:
        if info is None:
            return (0, None, None)
        info = coerce_annotation_info(info)
        color = None if info.color is None else _normalize_colors(info.color).tolist()
        return (info.id, info.label, color)

    def keypoint_connections_to_flat_list(
        keypoint_connections: Optional[Iterable[Union[int, Tuple[int, int]]]]
    ) -> Sequence[int]:
        if keypoint_connections is None:
            return []
        # flatten keypoint connections
        connections = list(keypoint_connections)
        if type(connections[0]) is tuple:
            connections = [item for tuple in connections for item in tuple]  # type: ignore[union-attr]
        return connections  # type: ignore[return-value]

    tuple_class_descriptions = [
        (
            info_to_tuple(d.info),
            tuple(info_to_tuple(a) for a in d.keypoint_annotations or []),
            keypoint_connections_to_flat_list(d.keypoint_connections),
        )
        for d in typed_class_descriptions
    ]

    # AnnotationContext arrow handling happens inside the python bridge
    bindings.log_annotation_context(obj_path, tuple_class_descriptions, timeless)
