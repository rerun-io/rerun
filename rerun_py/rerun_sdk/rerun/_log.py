from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path
from typing import TYPE_CHECKING, Any

import rerun_bindings as bindings

from ._baseclasses import AsComponents  # noqa: TC001
from .error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    import pyarrow as pa

    from ._baseclasses import ComponentDescriptor, DescribedComponentBatch
    from .recording_stream import RecordingStream


@catch_and_log_exceptions()
def log(
    entity_path: str | list[object],
    entity: AsComponents | Iterable[DescribedComponentBatch],
    *extra: AsComponents | Iterable[DescribedComponentBatch],
    static: bool = False,
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    r"""
    Log data to Rerun.

    This is the main entry point for logging data to rerun. It can be used to log anything
    that implements the [`rerun.AsComponents`][] interface, or a collection of `ComponentBatchLike`
    objects.

    When logging data, you must always provide an [entity_path](https://www.rerun.io/docs/concepts/entity-path)
    for identifying the data. Note that paths prefixed with "__" are considered reserved for use by the Rerun SDK
    itself and should not be used for logging user data. This is where Rerun will log additional information
    such as properties and warnings.

    The most common way to log is with one of the rerun archetypes, all of which implement
    the `AsComponents` interface.

    For example, to log a 3D point:
    ```py
    rr.log("my/point", rr.Points3D(position=[1.0, 2.0, 3.0]))
    ```

    The `log` function can flexibly accept an arbitrary number of additional objects which will
    be merged into the first entity so long as they don't expose conflicting components, for instance:
    ```py
    # Log three points with arrows sticking out of them,
    # and a custom "confidence" component.
    rr.log(
        "my/points",
        rr.Points3D([[0.2, 0.5, 0.3], [0.9, 1.2, 0.1], [1.0, 4.2, 0.3]], radii=[0.1, 0.2, 0.3]),
        rr.Arrows3D(vectors=[[0.3, 2.1, 0.2], [0.9, -1.1, 2.3], [-0.4, 0.5, 2.9]]),
        rr.AnyValues(confidence=[0.3, 0.4, 0.9]),
    )
    ```

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.

        The entity path can either be a string
        (with special characters escaped, split on unescaped slashes)
        or a list of unescaped strings.
        This means that logging to `"world/my\ image\!"` is the same as logging
        to ["world", "my image!"].

        See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.

    entity:
        Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype,
        or an iterable of (described)component batches.

    *extra:
        An arbitrary number of additional component bundles implementing the [`rerun.AsComponents`][]
        interface, that are logged to the same entity path.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        if None, use the global default from `rerun.strict_mode()`

    """

    # TODO(jleibs): Profile is_instance with runtime_checkable vs has_attr
    # Note from: https://docs.python.org/3/library/typing.html#typing.runtime_checkable
    #
    # An isinstance() check against a runtime-checkable protocol can be
    # surprisingly slow compared to an isinstance() check against a non-protocol
    # class. Consider using alternative idioms such as hasattr() calls for
    # structural checks in performance-sensitive code. hasattr is
    if hasattr(entity, "as_component_batches"):
        components = list(entity.as_component_batches())
    elif isinstance(entity, Iterable):
        components = list(entity)
    else:
        raise TypeError(
            f"Expected an object implementing rerun.AsComponents or an iterable of rerun.DescribedComponentBatch, "
            f"but got {type(entity)} instead.",
        )

    for ext in extra:
        if hasattr(ext, "as_component_batches"):
            components.extend(ext.as_component_batches())
        elif isinstance(ext, Iterable):
            components.extend(ext)
        else:
            raise TypeError(
                f"Expected an object implementing rerun.AsComponents or an iterable of rerun.DescribedComponentBatch, "
                f"but got {type(entity)} instead.",
            )

    _log_components(
        entity_path=entity_path,
        components=components,
        static=static,
        recording=recording,  # NOLINT
    )


def _log_components(
    entity_path: str | list[object],
    components: list[DescribedComponentBatch],
    *,
    static: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Internal method to log an entity from a collection of `ComponentBatchLike` objects.

    See also: [`rerun.log`][].

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.

        The entity path can either be a string
        (with special characters escaped, split on unescaped slashes)
        or a list of unescaped strings.
        This means that logging to `"world/my\ image\!"` is the same as logging
        to ["world", "my image!"].

        See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.

    components:
        A collection of `ComponentBatchLike` objects.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    instanced: dict[ComponentDescriptor, pa.Array] = {}

    descriptors = [comp.component_descriptor() for comp in components]
    arrow_arrays = [comp.as_arrow_array() for comp in components]

    if isinstance(entity_path, list):
        entity_path = bindings.new_entity_path([str(part) for part in entity_path])

    added = set()

    for descr, array in zip(descriptors, arrow_arrays):
        # Array could be None if there was an error producing the empty array
        # Nothing we can do at this point other than ignore it. Some form of error
        # should have been logged.
        if array is None:
            continue

        # Skip components which were logged multiple times.
        if descr in added:
            _send_warning_or_raise(
                f"Component {descr} was included multiple times. Only the first instance will be used.",
                depth_to_user_code=1,
            )
            continue
        else:
            added.add(descr)

        instanced[descr] = array

    bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
        entity_path,
        components=instanced,
        static_=static,
        recording=recording.to_native() if recording is not None else None,
    )


# TODO(#3841): expose timepoint settings once we implement stateless APIs
@catch_and_log_exceptions()
def log_file_from_path(
    file_path: str | Path,
    *,
    entity_path_prefix: str | None = None,
    static: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Logs the file at the given `path` using all `DataLoader`s available.

    A single `path` might be handled by more than one loader.

    This method blocks until either at least one `DataLoader` starts
    streaming data in or all of them fail.

    See <https://www.rerun.io/docs/getting-started/data-in/open-any-file> for more information.

    Parameters
    ----------
    file_path:
        Path to the file to be logged.

    entity_path_prefix:
        What should the logged entity paths be prefixed with?

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    bindings.log_file_from_path(
        Path(file_path),
        entity_path_prefix=entity_path_prefix,
        static_=static,
        recording=recording.to_native() if recording is not None else None,
    )


# TODO(cmc): expose timepoint settings once we implement stateless APIs
@catch_and_log_exceptions()
def log_file_from_contents(
    file_path: str | Path,
    file_contents: bytes,
    *,
    entity_path_prefix: str | None = None,
    static: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    r"""
    Logs the given `file_contents` using all `DataLoader`s available.

    A single `path` might be handled by more than one loader.

    This method blocks until either at least one `DataLoader` starts
    streaming data in or all of them fail.

    See <https://www.rerun.io/docs/getting-started/data-in/open-any-file> for more information.

    Parameters
    ----------
    file_path:
        Path to the file that the `file_contents` belong to.

    file_contents:
        Contents to be logged.

    entity_path_prefix:
        What should the logged entity paths be prefixed with?

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.log_file_from_contents(
        Path(file_path),
        file_contents,
        entity_path_prefix=entity_path_prefix,
        static_=static,
        recording=recording.to_native() if recording is not None else None,
    )


def escape_entity_path_part(part: str) -> str:
    r"""
    Escape an individual part of an entity path.

    For instance, `escape_entity_path_path("my image!")` will return `"my\ image\!"`.

    See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.

    Parameters
    ----------
    part:
        An unescaped string

    Returns
    -------
    str:
        The escaped entity path.

    """
    return str(bindings.escape_entity_path_part(part))


def new_entity_path(entity_path: list[Any]) -> str:
    r"""
    Construct an entity path, defined by a list of (unescaped) parts.

    If any part if not a string, it will be converted to a string using `str()`.

    For instance, `new_entity_path(["world", 42, "my image!"])` will return `"world/42/my\ image\!"`.

    See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.

    Parameters
    ----------
    entity_path:
        A list of strings to escape and join with slash.

    Returns
    -------
    str:
        The escaped entity path.

    """
    return str(bindings.new_entity_path([str(part) for part in entity_path]))
