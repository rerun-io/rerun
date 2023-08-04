from __future__ import annotations

from typing import Iterable

from rerun._rerun2.datatypes import AnnotationInfo, ClassDescription, ClassDescriptionLike
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = ["log_annotation_context", "AnnotationInfo", "ClassDescription", "ClassDescriptionLike"]


@log_decorator
def log_annotation_context(
    entity_path: str,
    class_descriptions: ClassDescriptionLike | Iterable[ClassDescriptionLike],
    *,
    timeless: bool = True,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an annotation context made up of a collection of [ClassDescription][rerun.log.annotation.ClassDescription]s.

    Any entity needing to access the annotation context will find it by searching the
    path upward. If all entities share the same you can simply log it to the
    root ("/"), or if you want a per-entity ClassDescriptions log it to the same path as
    your entity.

    Each ClassDescription must include an annotation info with an id, which will
    be used for matching the class and may optionally include a label and color.
    Colors should either be in 0-255 gamma space or in 0-1 gamma space. Colors
    can be RGB or RGBA.

    These can either be specified verbosely as:
    ```
    [AnnotationInfo(id=23, label='foo', color=(255, 0, 0)), ...]
    ```

    Or using short-hand tuples.
    ```
    [(23, 'bar'), ...]
    ```

    Unspecified colors will be filled in by the visualizer randomly.

    Parameters
    ----------
    entity_path:
        The path to the annotation context in the space hierarchy.
    class_descriptions:
        A single ClassDescription or a collection of ClassDescriptions.
    timeless:
        If true, the annotation context will be timeless (default: True).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    import rerun.experimental as rr2

    recording = RecordingStream.to_native(recording)

    rr2.log(entity_path, rr2.AnnotationContext(class_descriptions), timeless=timeless, recording=recording)
