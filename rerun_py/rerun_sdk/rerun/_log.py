from __future__ import annotations

from pathlib import Path
from typing import Any, Iterable

import pyarrow as pa
import rerun_bindings as bindings

from ._baseclasses import AsComponents, ComponentBatchLike
from .error_utils import _send_warning_or_raise, catch_and_log_exceptions
from .recording_stream import RecordingStream


class IndicatorComponentBatch:
    """
    A batch of Indicator Components that can be included in a Bundle.

    Indicator Components signal that a given Bundle should prefer to be interpreted as a
    given archetype. This helps the view heuristics choose the correct view in situations
    where multiple archetypes would otherwise be overlapping.

    This implements the `ComponentBatchLike` interface.
    """

    data: pa.Array

    def __init__(self, archetype_name: str) -> None:
        """
        Creates a new indicator component based on a given `archetype_name`.

        Parameters
        ----------
        archetype_name:
            The fully qualified name of the Archetype.

        """
        self.data = pa.nulls(1, type=pa.null())
        self._archetype_name = archetype_name

    def component_name(self) -> str:
        return self._archetype_name.replace("archetypes", "components") + "Indicator"

    def as_arrow_array(self) -> pa.Array:
        return self.data


@catch_and_log_exceptions()
def log(
    entity_path: str | list[str],
    entity: AsComponents | Iterable[ComponentBatchLike],
    *extra: AsComponents | Iterable[ComponentBatchLike],
    timeless: bool = False,
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
    for identifying the data. Note that the path prefix "rerun/" is considered reserved for use by the Rerun SDK
    itself and should not be used for logging user data. This is where Rerun will log additional information
    such as warnings.

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

    See also: [`rerun.log_components`][].

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
        Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype.

    *extra:
        An arbitrary number of additional component bundles implementing the [`rerun.AsComponents`][]
        interface, that are logged to the same entity path.

    timeless:
        Deprecated. Refer to `static` instead.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time_sequence`][], [`rerun.set_time_seconds`][] or
        [`rerun.set_time_nanos`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        if None, use the global default from `rerun.strict_mode()`

    """

    if timeless is True:
        import warnings

        warnings.warn(
            message=("`timeless` is deprecated as an argument to `log`; prefer `static` instead"),
            category=DeprecationWarning,
        )
        static = True

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
            f"Expected an object implementing rerun.AsComponents or an iterable of rerun.ComponentBatchLike, "
            f"but got {type(entity)} instead."
        )

    for ext in extra:
        if hasattr(ext, "as_component_batches"):
            components.extend(ext.as_component_batches())
        elif isinstance(ext, Iterable):
            components.extend(ext)
        else:
            raise TypeError(
                f"Expected an object implementing rerun.AsComponents or an iterable of rerun.ComponentBatchLike, "
                f"but got {type(entity)} instead."
            )

    if hasattr(entity, "num_instances"):
        num_instances = entity.num_instances()
    else:
        num_instances = None

    log_components(
        entity_path=entity_path,
        components=components,
        num_instances=num_instances,
        static=static,
        recording=recording,
    )


@catch_and_log_exceptions()
def log_components(
    entity_path: str | list[str],
    components: Iterable[ComponentBatchLike],
    *,
    num_instances: int | None = None,
    timeless: bool = False,
    static: bool = False,
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    r"""
    Log an entity from a collection of `ComponentBatchLike` objects.

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
        A collection of `ComponentBatchLike` objects that

    num_instances:
        Optional. The number of instances in each batch. If not provided, the max of all
        components will be used instead.

    timeless:
        Deprecated. Refer to `static` instead.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time_sequence`][], [`rerun.set_time_seconds`][] or
        [`rerun.set_time_nanos`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        if None, use the global default from `rerun.strict_mode()`

    """

    if timeless is True:
        import warnings

        warnings.warn(
            message=("`timeless` is deprecated as an argument to `log`; prefer `static` instead"),
            category=DeprecationWarning,
        )
        static = True

    # Convert to a native recording
    recording = RecordingStream.to_native(recording)

    instanced: dict[str, pa.Array] = {}

    components = list(components)

    names = [comp.component_name() for comp in components]
    arrow_arrays = [comp.as_arrow_array() for comp in components]

    if num_instances is None:
        num_instances = max(len(arr) for arr in arrow_arrays)

    if isinstance(entity_path, list):
        entity_path = bindings.new_entity_path([str(part) for part in entity_path])

    added = set()

    for name, array in zip(names, arrow_arrays):
        # Array could be None if there was an error producing the empty array
        # Nothing we can do at this point other than ignore it. Some form of error
        # should have been logged.
        if array is None:
            continue

        # Skip components which were logged multiple times.
        if name in added:
            _send_warning_or_raise(
                f"Component {name} was included multiple times. Only the first instance will be used.",
                depth_to_user_code=1,
            )
            continue
        else:
            added.add(name)

        # Strip off the ExtensionArray if it's present. We will always log via component_name.
        # TODO(jleibs): Maybe warn if there is a name mismatch here.
        if isinstance(array, pa.ExtensionArray):
            array = array.storage

        instanced[name] = array

    bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
        entity_path,
        components=instanced,
        static_=static,
        recording=recording,
    )


# TODO(#3841): expose timepoint settings once we implement stateless APIs
@catch_and_log_exceptions()
def log_file_from_path(
    file_path: str | Path,
    *,
    entity_path_prefix: str | None = None,
    static: bool = False,
    timeless: bool = False,
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

    timeless:
        Deprecated. Refer to `static` instead.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time_sequence`][], [`rerun.set_time_seconds`][] or
        [`rerun.set_time_nanos`][] will also be included.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    if timeless is True:
        import warnings

        warnings.warn(
            message=("`timeless` is deprecated as an argument to `log`; prefer `static` instead"),
            category=DeprecationWarning,
        )
        static = True

    bindings.log_file_from_path(
        Path(file_path),
        entity_path_prefix=entity_path_prefix,
        static_=static,
        recording=recording,
    )


# TODO(cmc): expose timepoint settings once we implement stateless APIs
@catch_and_log_exceptions()
def log_file_from_contents(
    file_path: str | Path,
    file_contents: bytes,
    *,
    entity_path_prefix: str | None = None,
    static: bool = False,
    timeless: bool | None = None,
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

    timeless:
        Deprecated. Refer to `static` instead.

    static:
        If true, the components will be logged as static data.

        Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        any temporal data of the same type.

        Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        Additional timelines set by [`rerun.set_time_sequence`][], [`rerun.set_time_seconds`][] or
        [`rerun.set_time_nanos`][] will also be included.

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
        recording=recording,
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
