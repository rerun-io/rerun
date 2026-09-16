# The Python surface of the `ViewerControlService` API, defined by
# `crates/store/re_protos/proto/rerun/v1alpha1/viewer_control.proto`.
# That file lists every place an operation has to be added.

from __future__ import annotations

import json
import os
import signal
import subprocess
import warnings
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, overload

from rerun._arrow import to_record_batch
from rerun.time import to_nanos, to_nanos_since_epoch

if TYPE_CHECKING:
    from collections.abc import Sequence
    from datetime import datetime, timedelta
    from types import TracebackType
    from uuid import UUID

    import datafusion
    import numpy as np
    import pyarrow as pa

    from rerun_bindings import ViewerClientInternal


_DEFAULT_URL = "rerun+http://127.0.0.1:9876/proxy"


StoreId = str
"""
Identifies one recording open in the viewer, as `{kind}:{application_id}:{recording_id}`.

`kind` is `Recording` or `Blueprint`. The application id is the application that logged it, or
the dataset id for a catalog-backed recording; the recording id is the recording itself, or its
segment id. Pass the whole string back to the methods that take a recording.

Both ids may contain a colon, so the application id's are escaped as `\\:` (and a backslash as
`\\\\`): the kind runs to the first colon, the application id to the next unescaped one, and the
recording id is the rest.
"""


@dataclass
class Timeline:
    """One timeline of a recording, with the range of times it holds."""

    name: str
    """Name of the timeline, e.g. `log_time`."""

    time_type: str
    """`sequence`, `duration`, or `timestamp`."""

    start: int | None
    """First time on the timeline, or None if it holds no data yet."""

    end: int | None
    """Last time on the timeline, or None if it holds no data yet."""


@dataclass
class Recording:
    """One recording open in the viewer."""

    store_id: StoreId
    timelines: list[Timeline]

    current_timeline: str | None
    """The timeline the cursor sits on, if any."""

    current_time: int | None
    """Where the cursor sits on that timeline, if set."""


@dataclass
class ViewReport:
    """A warning or an error a view reported the last time it was shown."""

    severity: str
    """`warning` or `error`."""

    summary: str
    details: str | None


@dataclass
class View:
    """One view of the viewer's current blueprint."""

    view_id: str
    view_class: str
    name: str
    origin: str
    visible: bool

    reports: list[ViewReport]
    """What failed to visualize. Empty when the view is healthy."""


@dataclass
class LoadingSource:
    """A data source the viewer is still loading from."""

    name: str
    """What is being loaded: a file path, a URL's display name, or a segment id."""

    status: str
    """The same thing the viewer's own loading screen says, e.g. `Loading /path/to/dataset…`."""


@dataclass
class ViewerState:
    """A snapshot of what the viewer is currently showing."""

    url: str
    """The current page, as a sharable URL. Empty for a page that has none."""

    active_recording: StoreId | None
    recordings: list[Recording]
    views: list[View]

    loading: list[LoadingSource]
    """
    What the viewer is still loading, empty once everything has arrived.

    `open_url` returns as soon as the load starts, and a recording appears in `recordings` as soon
    as its first message lands, so a recording with no timelines yet means "still arriving" rather
    than "empty". Poll until this is empty before concluding that a load finished.
    """

    catalog_url: str | None
    """
    Origin of the catalog server the viewer hosts.

    Hand this to [`CatalogClient`][rerun.catalog.CatalogClient] to read the data behind the open
    recordings; this API drives the viewer and deliberately does not serve data itself.
    """

    viewer_version: str | None
    """
    Version of the viewer answering, e.g. `0.38.0-alpha.1`.

    Which Rerun this is decides which API and which docs apply, so read it here rather than
    shelling out to `rerun --version` and hoping it found the same binary.
    """


@dataclass
class LogEntry:
    """One message the viewer logged."""

    sequence: int
    """Increases by one per message. Pass the last one back to fetch only what is new."""

    level: str
    """`INFO`, `WARN`, or `ERROR`."""

    target: str
    """The module that logged it, starting with the crate name."""

    message: str


def _time_type(raw: str | None) -> str:
    """Turn `TIME_TYPE_TIMESTAMP_NS` into `timestamp`, and the like."""
    return (raw or "").removeprefix("TIME_TYPE_").removesuffix("_NS").lower()


def _int_field(raw: dict[str, Any] | None, name: str) -> int | None:
    """
    Read an integer out of an optional message.

    Canonical protobuf JSON omits a scalar that holds its default, so a present-but-zero value
    arrives as a missing key. Absent means the enclosing message was unset; zero means it was set
    and happens to be zero.
    """
    if raw is None:
        return None
    return int(raw.get(name, 0))


def _viewer_state_from_json(raw: dict[str, Any]) -> ViewerState:
    """
    Build a `ViewerState` out of the canonical protobuf JSON the bindings return.

    Separate from the call that fetches it, so the shape can be tested without a viewer.
    """

    recordings = []
    for recording in raw.get("recordings", []):
        cursor = recording.get("current_time") or {}
        recordings.append(
            Recording(
                store_id=recording.get("store_id", ""),
                timelines=[
                    Timeline(
                        name=timeline.get("timeline", {}).get("name", ""),
                        time_type=_time_type(timeline.get("time_type")),
                        start=_int_field(timeline.get("time_range"), "start"),
                        end=_int_field(timeline.get("time_range"), "end"),
                    )
                    for timeline in recording.get("timelines", [])
                ],
                current_timeline=cursor.get("timeline", {}).get("name"),
                current_time=_int_field(cursor.get("time"), "time"),
            )
        )

    views = [
        View(
            view_id=view.get("view_id", ""),
            view_class=view.get("class", ""),
            name=view.get("name", ""),
            origin=view.get("origin", ""),
            visible=view.get("visible", False),
            reports=[
                ViewReport(
                    severity=report.get("severity", ""),
                    summary=report.get("summary", ""),
                    details=report.get("details"),
                )
                for report in view.get("reports", [])
            ],
        )
        for view in raw.get("views", [])
    ]

    loading = [
        LoadingSource(name=source.get("name", ""), status=source.get("status", "")) for source in raw.get("loading", [])
    ]

    return ViewerState(
        url=raw.get("url", ""),
        active_recording=raw.get("active_store_id"),
        recordings=recordings,
        views=views,
        loading=loading,
        catalog_url=raw.get("catalog_url"),
        viewer_version=raw.get("viewer_version"),
    )


class ViewerClient:
    """
    A connection to an instance of a Rerun viewer.

    Use the [`connect`][rerun.experimental.ViewerClient.connect] classmethod
    to attach to an already-running viewer, or
    [`spawn`][rerun.experimental.ViewerClient.spawn] to start a fresh one
    (e.g. in headless mode for CI screenshots).

    Spawned-viewer teardown:

    - Explicit [`close`][rerun.experimental.ViewerClient.close] always
      terminates the spawned viewer.
    - For an attached viewer (`detach_process=False`), exiting a `with` block
      or garbage-collecting the client also terminates the viewer.
    - A detached viewer keeps running through `with` exits and garbage
      collection. Only an explicit `close()` shuts it down.

    !!! warning
        This API is experimental and may change or be removed in future versions.
    """

    def __init__(
        self,
        url: str = _DEFAULT_URL,
        *,
        _pid: int | None = None,
        _kill_on_exit: bool = False,
    ) -> None:
        """
        Low-level constructor.

        Prefer
        [`ViewerClient.connect`][rerun.experimental.ViewerClient.connect] or
        [`ViewerClient.spawn`][rerun.experimental.ViewerClient.spawn].

        Parameters
        ----------
        url:
            The URL to connect to. The scheme must be one of `rerun://`,
            `rerun+http://`, or `rerun+https://`, and the pathname must be
            `/proxy` — the same form accepted by [`rerun.connect_grpc`][].
            Defaults to `rerun+http://127.0.0.1:9876/proxy`.
        _pid:
            Internal — set by `spawn()` to the pid of the launched viewer so
            that `close()` can terminate it.
        _kill_on_exit:
            Internal — set by `spawn()` to indicate that implicit teardown
            (`__exit__`, `__del__`) should call `close()`. See the class
            docstring for the full teardown rules.

        """
        from rerun_bindings import ViewerClientInternal

        # `close()` kills the spawned viewer when `_pid` is set. Implicit
        # teardown via `__exit__` or `__del__` is additionally gated on
        # `_kill_on_exit`: a detached viewer is meant to survive both.
        self._pid: int | None = _pid
        self._kill_on_exit: bool = _kill_on_exit
        self._url: str = url
        self._internal: ViewerClientInternal = ViewerClientInternal(url)

    @classmethod
    def connect(cls, url: str = _DEFAULT_URL) -> ViewerClient:
        """
        Connect to an already-running viewer.

        Parameters
        ----------
        url:
            The URL to connect to. The scheme must be one of `rerun://`,
            `rerun+http://`, or `rerun+https://`, and the pathname must be
            `/proxy` — the same form accepted by [`rerun.connect_grpc`][].
            Defaults to `rerun+http://127.0.0.1:9876/proxy`.

        """
        return cls(url)

    @classmethod
    def spawn(
        cls,
        *,
        headless: bool = False,
        port: int = 9876,
        memory_limit: str = "75%",
        server_memory_limit: str = "1GiB",
        hide_welcome_screen: bool = False,
        detach_process: bool | None = None,
        executable_name: str = "rerun",
        executable_path: str | None = None,
    ) -> ViewerClient:
        """
        Spawn a fresh viewer process and connect to it.

        Parameters
        ----------
        headless:
            Run the spawned viewer in headless mode (no OS window).
            The viewer still listens for gRPC connections, so the SDK can keep
            logging data and request screenshots via
            [`save_screenshot`][rerun.experimental.ViewerClient.save_screenshot].

            A working graphics stack must be present — either a real GPU/driver or a
            software rasterizer like Mesa's `lavapipe`. In a bare CI
            container with no Vulkan adapter, the viewer panics on
            startup with "No graphics adapter was found".
        port:
            The port to listen on.
        memory_limit:
            An upper limit on how much memory the Rerun Viewer should use.
            When this limit is reached, Rerun will drop the oldest data.
            Example: `16GB` or `50%` (of system total).
        server_memory_limit:
            An upper limit on how much memory the gRPC server running
            in the same process as the Rerun Viewer should use.
            When this limit is reached, Rerun will drop the oldest data.
            Example: `16GB` or `50%` (of system total).

            Defaults to `1GiB`.
        hide_welcome_screen:
            Hide the normal Rerun welcome screen.
        detach_process:
            Detach the spawned viewer from this Python process.

            A detached viewer survives unexpected parent termination
            (e.g. crashes or terminal hang-up), `with` block exits, and
            garbage collection — to take it down you must call
            [`close`][rerun.experimental.ViewerClient.close] explicitly.
            An attached viewer is killed by all of those.

            Defaults to `True` for a regular GUI viewer and `False` when
            `headless=True`, since a leftover invisible viewer is rarely what
            you want.
        executable_name:
            Specifies the name of the Rerun executable.
            You can omit the `.exe` suffix on Windows.

            Defaults to `rerun`.
        executable_path:
            Enforce a specific executable to use instead of searching
            through PATH for `executable_name`.

            Unspecified by default.

        """
        from rerun._spawn import _spawn_viewer

        if detach_process is None:
            detach_process = not headless

        pid = _spawn_viewer(
            port=port,
            memory_limit=memory_limit,
            server_memory_limit=server_memory_limit,
            hide_welcome_screen=hide_welcome_screen,
            detach_process=detach_process,
            executable_name=executable_name,
            executable_path=executable_path,
            headless=headless,
        )
        return cls(
            f"rerun+http://127.0.0.1:{port}/proxy",
            _pid=pid,
            _kill_on_exit=not detach_process,
        )

    @property
    def url(self) -> str:
        """The `rerun+http://…/proxy` URL of the viewer this client is connected to."""
        return self._url

    def send_table(self, name: str, table: pa.RecordBatch | list[pa.RecordBatch] | datafusion.DataFrame) -> None:
        """
        Send a table to the viewer.

        A table is represented as a dataframe defined by an Arrow record batch.

        Parameters
        ----------
        name:
            The table name.

            !!! note
                The table name serves as an identifier.
                If you send a table with the same name twice, the second table will replace the first one.

        table:
            The Arrow RecordBatch containing the table data to send.

        """
        # TODO(RR-3481): we should be able to stream multiple record batches instead of having to merge to one. This
        # requires changing the grpc protocol though, or rolling a OSS server sidecar to the Viewer.
        self._internal.send_table(name, to_record_batch(table))

    def close_recordings(
        self,
        target: str | Sequence[StoreId] = "current",
    ) -> list[StoreId]:
        """
        Close recordings in the viewer, and return what was closed.

        This only removes them from the viewer. Files on disk are untouched, and registered
        recordings stay in the catalog and can be reopened, but unsaved blueprint edits are lost.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        target:
            `"current"` to close the active recording, `"all"` to close every open one, or the
            [`StoreId`][rerun.experimental.StoreId] of a recording to close, or several of them.
            `viewer_state()` reports the open recordings and their ids.

        """
        if target in ("current", "all"):
            assert isinstance(target, str)
            raw = json.loads(self._internal.close_recordings(target=target))
        else:
            store_ids = [target] if isinstance(target, str) else list(target)
            raw = json.loads(self._internal.close_recordings(store_ids=store_ids))
        return list(raw.get("closed", []))

    def open_url(self, url: str) -> None:
        """
        Open a URL in the viewer.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        url:
            A recording or blueprint file, a `rerun://` dataset URI, a redap server or catalog
            URL, or an intra-recording link.

        """
        self._internal.open_url(url)

    def save_screenshot(self, file_path: str, view_id: str | UUID | None = None) -> None:
        """
        Save a screenshot to a file.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        file_path:
            The path where the screenshot will be saved.

            !!! important
                This path is relative to the viewer's filesystem, not the client's.
                If your viewer runs on a different machine, the screenshot will be saved there.

        view_id:
            Optional view ID to screenshot.
            If None, screenshots the entire viewer.

        """
        view_id_str = str(view_id) if view_id is not None else None
        self._internal.save_screenshot(file_path, view_id_str)

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        sequence: int,
        play: bool = False,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        duration: int | float | timedelta | np.timedelta64,
        play: bool = False,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        timestamp: int | float | datetime | np.datetime64,
        play: bool = False,
        recording: StoreId | None = None,
    ) -> None: ...

    def set_time(
        self,
        timeline: str | None = None,
        *,
        sequence: int | None = None,
        duration: int | float | timedelta | np.timedelta64 | None = None,
        timestamp: int | float | datetime | np.datetime64 | None = None,
        play: bool = False,
        recording: StoreId | None = None,
    ) -> None:
        """
        Set the viewer's time cursor.

        Parameters
        ----------
        timeline:
            The timeline to seek on.
            If omitted, the viewer uses its active timeline.
        sequence:
            A sequence index.
        duration:
            A duration in seconds, or a duration value with nanosecond precision.
        timestamp:
            Seconds since Unix epoch, or a timestamp value with nanosecond precision.
        play:
            Start playing from the new position.
            The viewer pauses by default.
        recording:
            The recording to seek, as reported by `viewer_state()`.
            If omitted, the viewer uses its active recording.

        """
        if sum(value is not None for value in (sequence, duration, timestamp)) != 1:
            raise ValueError("ViewerClient.set_time expects exactly one of sequence, duration, or timestamp")

        if sequence is not None:
            time = sequence
        elif duration is not None:
            time = to_nanos(duration)
        else:
            assert timestamp is not None
            time = to_nanos_since_epoch(timestamp)

        self._internal.set_time_cursor(timeline, time, play, recording)

    def viewer_logs(self, after_sequence: int | None = None) -> list[LogEntry]:
        """
        Return the viewer's recent log messages, oldest first.

        The viewer keeps a bounded buffer, so old entries drop out.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        after_sequence:
            Only return entries newer than this sequence number.
            Pass the last one you saw to fetch only what is new. None returns everything buffered.

        """
        raw = json.loads(self._internal.viewer_logs(after_sequence))
        return [
            LogEntry(
                sequence=entry.get("sequence", 0),
                level=entry.get("level", ""),
                target=entry.get("target", ""),
                message=entry.get("message", ""),
            )
            for entry in raw.get("entries", [])
        ]

    def viewer_state(self) -> ViewerState:
        """
        Report what the viewer is currently showing.

        Call this to learn which recording and timeline to drive, and which time values are valid,
        before moving the time cursor. A view's reports say what failed to visualize.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        """
        return _viewer_state_from_json(json.loads(self._internal.viewer_state()))

    def close(self) -> None:
        """
        Close the client, terminating the spawned viewer.

        Emits a `UserWarning` and is a no-op if there is no spawned viewer to
        terminate (either the client never spawned one, or it has already
        been closed). Safe to call multiple times — only the first call has
        an effect.
        """
        pid = self._pid
        self._pid = None
        if pid is None:
            warnings.warn(
                "ViewerClient.close() called with no viewer to terminate "
                "(the client was constructed via ViewerClient.connect(), or close() was already called).",
                UserWarning,
                stacklevel=2,
            )
            return

        try:
            # The python `rerun` command is a shim (see `rerun_cli/__main__.py`) that spawns the
            # rust cli binary as a child process. Killing only the shim pid would orphan that child
            # and leak the viewer (along with the port it holds), so we must take down the whole
            # process tree.
            if os.name != "posix":
                # Windows has no POSIX process groups. `taskkill /T` walks the parent → child
                # relationship Windows records for the `subprocess.call` in the shim and kills the
                # native viewer too. `/F` is required because the GUI viewer has no console to
                # receive a graceful signal (`os.kill`/SIGTERM maps to `TerminateProcess` anyway).
                subprocess.run(
                    ["taskkill", "/PID", str(pid), "/T", "/F"],
                    check=True,
                    capture_output=True,
                )
            else:
                # On unix the shim is launched in its own process group (see `spawn.rs`), and the
                # viewer child inherits it, so we can kill both cleanly with a single `killpg`.
                os.killpg(pid, signal.SIGTERM)
        except (OSError, subprocess.CalledProcessError) as err:
            warnings.warn(
                f"ViewerClient.close() could not close pid {pid}: {err}",
                UserWarning,
                stacklevel=2,
            )

    def __enter__(self) -> ViewerClient:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        # Only attached viewers are torn down on `with` exit. Skip when
        # there's nothing to kill so we don't trip close()'s warning if the
        # user already closed manually inside the block.
        if self._kill_on_exit and self._pid is not None:
            self.close()

    def __del__(self) -> None:
        # Try stopping the viewer if it wasn't detached. Skip when there's
        # nothing to kill — both because there's no work to do and to avoid
        # tripping close()'s warning during GC.
        try:
            if not getattr(self, "_kill_on_exit", False):
                return
            if getattr(self, "_pid", None) is None:
                return
            self.close()
        except Exception:
            pass
