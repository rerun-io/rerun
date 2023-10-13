from __future__ import annotations

from typing import Iterable

import pyarrow as pa
import rerun_bindings as bindings

from . import components as cmp
from ._baseclasses import AsComponents, ComponentBatchLike
from .error_utils import _send_warning_or_raise, catch_and_log_exceptions
from .recording_stream import RecordingStream

__all__ = ["log", "IndicatorComponentBatch", "AsComponents"]


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


def _splat() -> cmp.InstanceKeyBatch:
    """Helper to generate a splat InstanceKeyArray."""

    _MAX_U64 = 2**64 - 1
    return pa.array([_MAX_U64], type=cmp.InstanceKeyType().storage_type)  # type: ignore[no-any-return]


@catch_and_log_exceptions()
def log(
    entity_path: str,
    entity: AsComponents | Iterable[ComponentBatchLike],
    *extra: AsComponents | Iterable[ComponentBatchLike],
    timeless: bool = False,
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    """
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
        rr.Points2D([[0.2, 0.5], [0.9, 1.2], [1.0, 4.2]], radii=[0.1, 0.2, 0.3]),
        rr.Arrows3D(vectors=[[0.3, 2.1], [0.2, -1.1], [-0.4, 0.1]]),
        rr.AnyValues(confidence=[0.3, 0.4, 0.9]),
    )
    ```

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.
    entity:
        Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype.
    *extra:
        An arbitrary number of additional component bundles implementing the [`rerun.AsComponents`][] interface, that are logged to the same entity path.
    timeless:
        If true, the entity will be timeless.

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

    See also: [`rerun.log_components`][].
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
        timeless=timeless,
        recording=recording,
    )


@catch_and_log_exceptions()
def log_components(
    entity_path: str,
    components: Iterable[ComponentBatchLike],
    *,
    num_instances: int | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
    strict: bool | None = None,
) -> None:
    """
    Log an entity from a collection of `ComponentBatchLike` objects.

    All of the batches should have the same length as the value of
    `num_instances`, or length 1 if the component is a splat., or 0 if the
    component is being cleared.

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.
    components:
        A collection of `ComponentBatchLike` objects that
    num_instances:
        Optional. The number of instances in each batch. If not provided, the max of all
        components will be used instead.
    timeless:
        If true, the entity will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    strict:
        If True, raise exceptions on non-loggable data.
        If False, warn on non-loggable data.
        if None, use the global default from `rerun.strict_mode()`

    See also: [`rerun.log`][].
    """
    instanced: dict[str, pa.Array] = {}
    splats: dict[str, pa.Array] = {}

    components = list(components)

    names = [comp.component_name() for comp in components]
    arrow_arrays = [comp.as_arrow_array() for comp in components]

    if num_instances is None:
        num_instances = max(len(arr) for arr in arrow_arrays)

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

        if len(array) == 1 and num_instances > 1:
            splats[name] = array
        else:
            instanced[name] = array

    if splats:
        splats["rerun.components.InstanceKey"] = _splat()
        bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
            entity_path,
            components=splats,
            timeless=timeless,
            recording=recording,
        )

    # Always log the instanced components last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
        entity_path,
        components=instanced,
        timeless=timeless,
        recording=recording,
    )
