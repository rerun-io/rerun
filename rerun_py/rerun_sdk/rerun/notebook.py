"""Helper functions for converting streams to inline html."""

from __future__ import annotations

import os
from datetime import datetime, timedelta
from typing import TYPE_CHECKING, Callable, Literal

import numpy as np
import pyarrow
import pyarrow.ipc as ipc
from pyarrow import RecordBatch

from .error_utils import deprecated_param
from .time import to_nanos, to_nanos_since_epoch

if TYPE_CHECKING:
    from .blueprint import BlueprintLike


from rerun import bindings
from rerun_notebook import ErrorWidget as _ErrorWidget, Viewer as _Viewer

from .event import (
    ViewerEvent as ViewerEvent,
    _viewer_event_from_json_str,
)
from .recording_stream import RecordingStream, get_data_recording

_default_width = 640
_default_height = 480

_Panel = Literal["top", "blueprint", "selection", "time"]
_PanelState = Literal["expanded", "collapsed", "hidden"]


def set_default_size(*, width: int | None, height: int | None) -> None:
    """
    Set the default size for the viewer.

    This will be used for any viewers created after this call.

    Parameters
    ----------
    width : int
        The width of the viewer in pixels.
    height : int
        The height of the viewer in pixels.

    """

    global _default_width, _default_height
    if width is not None:
        _default_width = width
    if height is not None:
        _default_height = height


class Viewer:
    """
    A viewer embeddable in a notebook.

    This viewer is a wrapper around the [`rerun_notebook.Viewer`][] widget.
    """

    def __init__(
        self,
        *,
        width: int | Literal["auto"] | None = None,
        height: int | Literal["auto"] | None = None,
        url: str | None = None,
        blueprint: BlueprintLike | None = None,
        recording: RecordingStream | None = None,
        use_global_recording: bool | None = None,
    ) -> None:
        """
        Create a new Rerun viewer widget for use in a notebook.

        Any data logged to the recording after initialization will be sent directly to the viewer.

        This widget can be displayed by returning it at the end of your cells execution, or immediately
        by calling [`Viewer.display`][].

        Parameters
        ----------
        width:
            The width of the viewer in pixels, or "auto".

            When set to "auto", scales to 100% of the notebook cell's width.
        height:
            The height of the viewer in pixels, or "auto".

            When set to "auto", scales using a 16:9 aspect ratio with `width`.
        url:
            Optional URL passed to the viewer for displaying its contents.
        recording:
            Specifies the [`rerun.RecordingStream`][] to use.
            If left unspecified, defaults to the current active data recording, if there is one.
            See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
        blueprint:
            A blueprint object to send to the viewer.
            It will be made active and set as the default blueprint in the recording.

            Setting this is equivalent to calling [`rerun.send_blueprint`][] before initializing the viewer.
        use_global_recording:
            If no explicit `recording` is provided, the Viewer uses the thread-local/global recording created by `rr.init`
            or set explicitly via `rr.set_thread_local_data_recording`/`rr.set_global_data_recording`.

            Settings this to `False` causes the Viewer to not pick up the global recording.

            Defaults to `False` if `url` is provided, and `True` otherwise.

        """

        self._error_widget = _ErrorWidget()
        self._viewer = _Viewer(
            width=width if width is not None else _default_width,
            height=height if height is not None else _default_height,
            url=url,
            fallback_token=os.environ.get("REDAP_TOKEN", None),
        )

        # Viewer event handling
        self._event_callbacks: list[Callable[[ViewerEvent], None]] = []

        def on_raw_event(json_str: str) -> None:
            evt = _viewer_event_from_json_str(json_str)
            for callback in self._event_callbacks:
                callback(evt)

        self._viewer._on_raw_event(on_raw_event)

        # By default, we use the global recording only if no `url` is provided.
        if use_global_recording is None:
            use_global_recording = url is None

        if use_global_recording:
            recording = get_data_recording(recording)

        if recording is not None:
            bindings.set_callback_sink(
                recording=recording.to_native(),
                callback=self._flush_hook,
            )

        if blueprint is not None:
            if recording is not None:
                recording.send_blueprint(blueprint)
            else:
                raise ValueError(
                    "Can only set a blueprint if there's either an active recording or a recording passed in"
                )

    def add_recording(
        self,
        recording: RecordingStream | None = None,
        blueprint: BlueprintLike | None = None,
    ) -> None:
        """
        Adds a recording to the viewer.

        If no recording is specified, the current active recording will be used.

        NOTE: By default all calls to `rr.init()` will re-use the same recording_id, meaning
        that your recordings will be merged together. If you want to keep them separate, you
        should call `rr.init("my_app_id", recording_id=uuid.uuid4())`.

        Parameters
        ----------
        recording : RecordingStream
            Specifies the [`rerun.RecordingStream`][] to use.
            If left unspecified, defaults to the current active data recording, if there is one.
            See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
        blueprint : BlueprintLike
            A blueprint object to send to the viewer.
            It will be made active and set as the default blueprint in the recording.

            Setting this is equivalent to calling [`rerun.send_blueprint`][] before initializing the viewer.

        """
        recording = get_data_recording(recording)
        if recording is None:
            raise ValueError("No recording specified and no active recording found")

        bindings.set_callback_sink(
            recording=recording.to_native(),
            callback=self._flush_hook,
        )

        if blueprint is not None:
            recording.send_blueprint(blueprint)

    def _add_table_id(self, record_batch: RecordBatch, table_id: str) -> RecordBatch:
        # Get current schema
        schema = record_batch.schema
        schema = schema.with_metadata({b"__table_id": table_id})

        # Create new record batch with updated schema
        return RecordBatch.from_arrays(record_batch.columns, schema=schema)

    def set_application_blueprint(
        self, application_id: str, blueprint: BlueprintLike, make_active: bool = True, make_default: bool = True
    ) -> None:
        """
        Set the blueprint for the given application.

        Parameters
        ----------
        application_id:
            The ID of the application to set the blueprint for.
        blueprint:
            The blueprint to set for the application.
        make_active:
            Whether to make the blueprint active.
            If `True`, the blueprint will be set as the active blueprint for the application.
        make_default:
            Whether to make the blueprint the default blueprint for the application.
            If `True`, the blueprint will be set as the default blueprint for the application.

        """

        blueprint = blueprint.to_blueprint()

        blueprint_stream = RecordingStream._from_native(
            bindings.new_blueprint(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            ),
        )

        blueprint_stream.set_time("blueprint", sequence=0)
        blueprint._log_to_stream(blueprint_stream)

        bindings.set_callback_sink_blueprint(
            callback=self._flush_hook,
            make_active=make_active,
            make_default=make_default,
            blueprint_stream=blueprint_stream.to_native(),
        )

    def send_table(
        self,
        id: str,
        table: RecordBatch,
    ) -> None:
        """
        Sends a table in the form of a dataframe to the viewer.

        Parameters
        ----------
        id:
            The name that uniquely identifies the table in the viewer.
            This name will also be shown in the recording panel.
        table:
            The table as a single Arrow record batch.

        """
        new_table = self._add_table_id(table, id)
        sink = pyarrow.BufferOutputStream()
        writer = ipc.new_stream(sink, new_table.schema)
        writer.write_batch(new_table)
        writer.close()
        table_as_bytes = sink.getvalue().to_pybytes()
        self._viewer.send_table(table_as_bytes)

    def display(self, block_until_ready: bool = False) -> None:
        """
        Display the viewer in the notebook cell immediately.

        Parameters
        ----------
        block_until_ready : bool
            Whether to block until the viewer is ready to receive data. If this is `False`, the viewer
            will still be displayed, but logged data will likely be queued until the viewer becomes ready
            at the end of cell execution.

        """

        from IPython.display import display

        display(self._error_widget)
        display(self._viewer)

        if block_until_ready:
            self._viewer.block_until_ready()

    def _ipython_display_(self) -> None:
        self.display()

    def _flush_hook(self, data: bytes) -> None:
        self._viewer.send_rrd(data)

    def update_panels(
        self,
        *,
        top: _PanelState | Literal["default"] | None = None,
        blueprint: _PanelState | Literal["default"] | None = None,
        selection: _PanelState | Literal["default"] | None = None,
        time: _PanelState | Literal["default"] | None = None,
    ) -> None:
        """
        Partially update the state of panels in the viewer.

        Valid states are the strings `expanded`, `collapsed`, `hidden`, `default`, and the value `None`.

        Panels set to:
        - `None` will be unchanged.
        - `expanded` will be fully expanded, taking up the most space.
        - `collapsed` will be smaller and simpler, omitting some information.
        - `hidden` will be completely invisible, taking up no space.
        - `default` will be reset to the default state.

        The `collapsed` state is the same as the `hidden` state for panels
        which do not support the `collapsed` state.

        Setting the panel state using this function will also prevent the user
        from modifying that panel's state in the viewer.

        Parameters
        ----------
        top: str
            State of the panel, positioned on the top of the viewer.
        blueprint: str
            State of the blueprint panel, positioned on the left side of the viewer.
        selection: str
            State of the selection panel, positioned on the right side of the viewer.
        time: str
            State of the time panel, positioned on the bottom side of the viewer.

        """

        panel_states: dict[_Panel, _PanelState | Literal["default"]] = {}
        if top:
            panel_states["top"] = top
        if blueprint:
            panel_states["blueprint"] = blueprint
        if selection:
            panel_states["selection"] = selection
        if time:
            panel_states["time"] = time

        self._viewer.update_panel_states(panel_states)

    def set_active_recording(
        self,
        *,
        recording_id: str,
    ) -> None:
        """
        Set the active recording for the viewer.

        This is equivalent to clicking on the given recording in the blueprint panel.

        Parameters
        ----------
        recording_id: str
            The ID of the recording to set the viewer to.

            Using this requires setting an explicit recording ID when creating the recording.

        """

        self._viewer.set_active_recording(recording_id)

    def set_active_partition_url(
        self,
        *,
        url: str,
    ) -> None:
        """
        Set the active partition url for the viewer.

        Parameters
        ----------
        url: str
            The URL of the partition to set the viewer to.

        """

        self._viewer.set_active_partition_url(url)

    @deprecated_param("nanoseconds", use_instead="duration or timestamp", since="0.23.0")
    @deprecated_param("seconds", use_instead="duration or timestamp", since="0.23.0")
    def set_time_ctrl(
        self,
        *,
        sequence: int | None = None,
        duration: int | float | timedelta | np.timedelta64 | None = None,
        timestamp: int | float | datetime | np.datetime64 | None = None,
        timeline: str | None = None,
        play: bool = False,
        # Deprecated parameters:
        nanoseconds: int | None = None,
        seconds: float | None = None,
    ) -> None:
        """
        Set the time control for the viewer.

        You are expected to set at most ONE of the arguments `sequence`, `duration`, or `timestamp`.

        Parameters
        ----------
        sequence:
            Used for sequential indices, like `frame_nr`.
            Must be an integer.
        duration:
            Used for relative times, like `time_since_start`.
            Must either be in seconds, a [`datetime.timedelta`][], or [`numpy.timedelta64`][].
            For nanosecond precision, use `numpy.timedelta64(nanoseconds, 'ns')`.
        timestamp:
            Used for absolute time indices, like `capture_time`.
            Must either be in seconds since Unix epoch, a [`datetime.datetime`][], or [`numpy.datetime64`][].
            For nanosecond precision, use `numpy.datetime64(nanoseconds, 'ns')`.
        play:
            Whether to start playing from the specified time point. Defaults to paused.
        timeline:
            The name of the timeline to switch to. If not provided, time will remain on the current timeline.
        nanoseconds:
            DEPRECATED: Use `duration` or 'timestamp` instead, with "seconds" as the unit.
        seconds:
            DEPRECATED: Use `duration` or 'timestamp` instead.

        """

        # Handle deprecated parameters:
        if nanoseconds is not None:
            duration = 1e-9 * nanoseconds
        if seconds is not None:
            duration = seconds

        if sum(x is not None for x in (sequence, duration, timestamp)) > 1:
            raise ValueError(
                f"set_time_ctrl: Exactly one of `sequence`, `duration`, and `timestamp` must be set (timeline='{timeline}')",
            )

        if sequence is not None:
            time = sequence
        elif duration is not None:
            time = to_nanos(duration)
        elif timestamp is not None:
            time = to_nanos_since_epoch(timestamp)
        else:
            time = None

        self._viewer.set_time_ctrl(timeline, time, play)

    def on_event(self, callback: Callable[[ViewerEvent], None]) -> None:
        self._event_callbacks.append(callback)
