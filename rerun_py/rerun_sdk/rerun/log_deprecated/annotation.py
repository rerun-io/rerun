from __future__ import annotations

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import AnnotationContext
from rerun.datatypes import AnnotationInfo, ClassDescription, ClassDescriptionArrayLike
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = ["log_annotation_context", "AnnotationInfo", "ClassDescription", "ClassDescriptionArrayLike"]


@deprecated(
    """Please migrate to `rr.log(…, rr.AnnotationContext(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_annotation_context(
    entity_path: str,
    class_descriptions: ClassDescriptionArrayLike,
    *,
    timeless: bool = True,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an annotation context made up of a collection of [ClassDescription][rerun.log_deprecated.annotation.ClassDescription]s.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.AnnotationContext][]

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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

    recording = RecordingStream.to_native(recording)

    log(entity_path, AnnotationContext(class_descriptions), timeless=timeless, recording=recording)
