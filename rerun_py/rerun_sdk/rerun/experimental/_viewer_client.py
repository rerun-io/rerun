# The Python surface of the `ViewerControlService` API, defined by
# `crates/store/re_protos/proto/rerun/v1alpha1/viewer_control.proto`.
# That file lists every place an operation has to be added.

from __future__ import annotations

import base64
import json
import math
import os
import signal
import subprocess
import time
import warnings
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Literal, overload

import psutil
from typing_extensions import deprecated

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

    location: str
    """The location that shows this recording, for [`set_location`][rerun.experimental.ViewerClient.set_location]."""

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

    location: str
    """
    What the viewer is showing, as a Viewer URL. Empty if that has no location.

    Pass it to [`set_location`][rerun.experimental.ViewerClient.set_location] to come back to it later.
    """

    active_recording: StoreId | None
    recordings: list[Recording]
    views: list[View]

    loading: list[LoadingSource]
    """
    What the viewer is still loading, empty once everything has arrived.

    `open` waits a few seconds at most for the load to finish, and a recording appears in
    `recordings` as soon as its first message lands, so a recording with no timelines yet means
    "still arriving" rather than "empty". Poll until this is empty before concluding that a load
    finished.
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
class OpenResult:
    """What [`open`][rerun.experimental.ViewerClient.open] opened."""

    store_ids: list[StoreId]
    """The store ids of the recordings loaded from the URLs."""

    loading: list[str]
    """
    The URLs, as passed in, whose sources had not finished their initial load when the viewer answered.

    Empty once everything is loaded. Otherwise, poll [`get_state`][rerun.experimental.ViewerClient.get_state]
    until its `loading` list is empty.
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


@dataclass
class ComponentSchema:
    """A component logged on an entity."""

    component: str
    """The name to pass along in a query, e.g. `Points3D:positions`."""

    archetype: str | None
    """The archetype it was logged through, e.g. `rerun.archetypes.Points3D`, if any."""

    component_type: str | None
    """The semantic type of a value, e.g. `rerun.components.Position3D`, if any."""

    datatype: str
    """Arrow datatype of a single value, e.g. `FixedSizeList(3 x non-null Float32)`."""

    has_static: bool
    """
    Whether the component has ever been logged statically.

    It may also have temporal data, which takes precedence where it exists.
    """


@dataclass
class EntitySchema:
    """An entity, with every component ever logged on it."""

    entity_path: str
    """The entity path, e.g. `/world/robot/joints`."""

    components: list[ComponentSchema]
    """The components logged on this entity. Empty with `paths_only`."""


@dataclass
class RecordingSchema:
    """A recording's timelines, entities, and the components logged on them."""

    store_id: StoreId
    """The recording that was described."""

    timelines: list[Timeline]
    """The recording's timelines, with their time ranges."""

    entities: list[EntitySchema]
    """The matching entities in path order."""

    omitted_entities: int
    """How many matching entities got left out because of `max_entities`."""


@dataclass
class DatasetSchema:
    """The entities of a dataset's segments, and the components logged on them."""

    timelines: list[str]
    """The names of the dataset's timelines."""

    entities: list[EntitySchema]
    """The matching entities in path order."""

    omitted_entities: int
    """How many matching entities got left out because of `max_entities`."""

    asset_ids: list[str]
    """
    The ids of the dataset's assets.

    [`get_dataset_assets_schema`][rerun.experimental.ViewerClient.get_dataset_assets_schema] describes them.
    """


@dataclass
class DatasetAssetSchema:
    """An asset of a dataset: static data shared by its segments, such as a robot's meshes."""

    asset_id: str
    """The asset's id, which is its segment id in the dataset's asset dataset."""

    entities: list[EntitySchema]
    """The matching entities in path order."""

    omitted_entities: int
    """How many matching entities of this asset got left out because of `max_entities`."""


@dataclass
class TableColumnSchema:
    """A column of a table."""

    name: str
    """The Arrow field name, which table blueprints use, e.g. `rerun_segment_id`."""

    display_name: str
    """The name in the table's header."""

    datatype: str
    """Arrow datatype of the column, e.g. `Utf8`."""


@dataclass
class BlueprintInfo:
    """Which blueprint was read or written."""

    blueprint_id: str
    """The id of the blueprint store."""

    store_id: StoreId | None
    """The recording whose blueprint it is, if it is a recording's."""

    table_url: str | None
    """The table whose blueprint it is, if it is a table's that has a URL."""


@dataclass
class Blueprint(BlueprintInfo):
    """A whole blueprint, as [`get_blueprint`][rerun.experimental.ViewerClient.get_blueprint] reads it."""

    blueprint: dict[str, Any]
    """The blueprint, keyed by blueprint entity path, then archetype, then field."""


@dataclass
class Command:
    """A command palette command, as [`list_commands`][rerun.experimental.ViewerClient.list_commands] describes it."""

    id: str
    """The id for [`run_command`][rerun.experimental.ViewerClient.run_command]."""

    title: str
    """The name shown in the palette."""

    description: str
    """What the command does."""

    scope: str
    """What kind of thing it acts on: `global`, `recording`, `server`, or `table`."""

    shortcuts: list[str]
    """Keyboard shortcuts as the viewer shows them, e.g. `⌘K`. Primary first."""

    available: bool
    """Whether `run_command` would run it now."""

    blocking_native_modal: bool
    """Whether it opens a native modal, which blocks the viewer until a person closes it."""

    destructive: bool
    """Whether it discards data, blueprint edits, a server, or the viewer itself."""

    target: str | None
    """The store id, server origin, or table within `scope` that it would act on now, if any."""


@dataclass
class ProfileTrace:
    """A puffin profile that [`capture_profile_trace`][rerun.experimental.ViewerClient.capture_profile_trace] wrote."""

    num_frames: int
    """How many frames the file holds."""

    file_path: str
    """Absolute path of the written `.puffin` file."""


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
                location=recording.get("location", ""),
                timelines=[_timeline_from_json(timeline) for timeline in recording.get("timelines", [])],
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
        location=raw.get("location", ""),
        active_recording=raw.get("active_store_id"),
        recordings=recordings,
        views=views,
        loading=loading,
        catalog_url=raw.get("catalog_url"),
        viewer_version=raw.get("viewer_version"),
    )


def _timeline_from_json(timeline: dict[str, Any]) -> Timeline:
    return Timeline(
        name=timeline.get("timeline", {}).get("name", ""),
        time_type=_time_type(timeline.get("time_type")),
        start=_int_field(timeline.get("time_range"), "start"),
        end=_int_field(timeline.get("time_range"), "end"),
    )


def _entities_from_json(entities: list[dict[str, Any]]) -> list[EntitySchema]:
    return [
        EntitySchema(
            entity_path=entity.get("entity_path", ""),
            components=[
                ComponentSchema(
                    component=component.get("component", ""),
                    archetype=component.get("archetype"),
                    component_type=component.get("component_type"),
                    datatype=component.get("datatype", ""),
                    has_static=component.get("has_static", False),
                )
                for component in entity.get("components", [])
            ],
        )
        for entity in entities
    ]


def _command_from_json(command: dict[str, Any]) -> Command:
    return Command(
        id=command.get("id", ""),
        title=command.get("title", ""),
        description=command.get("description", ""),
        scope=command.get("scope", ""),
        shortcuts=command.get("shortcuts", []),
        available=command.get("available", False),
        blocking_native_modal=command.get("blocking_native_modal", False),
        destructive=command.get("destructive", False),
        target=command.get("target"),
    )


def _blueprint_target(
    of: Literal["recording", "table"] | None,
    store_id: StoreId | None,
    table_url: str | None,
) -> dict[str, Any]:
    """The `target` of a blueprint request: a recording's, a table's, or (without any) what is on screen."""
    if of is None:
        of = "recording" if store_id is not None else "table" if table_url is not None else None
    if (of == "recording" and table_url is not None) or (of == "table" and store_id is not None):
        raise ValueError("A recording's blueprint takes a store_id, and a table's a table_url")
    if of == "recording":
        return {"recording": _without_none(store_id=store_id)}
    if of == "table":
        return {"table": _without_none(table_url=table_url)}
    return {}


def _schema_request(entity_path: str | None, max_entities: int | None, paths_only: bool | None) -> dict[str, Any]:
    return _without_none(entity_path=entity_path, max_entities=max_entities, paths_only=paths_only)


def _without_none(**fields: Any) -> dict[str, Any]:
    """The fields that are set, since canonical protobuf JSON leaves out the unset ones."""
    return {name: value for name, value in fields.items() if value is not None}


def _wait_for_processes(processes: list[psutil.Process], timeout: float) -> list[psutil.Process]:
    """Reap direct children and wait for descendants to exit, excluding orphan zombies."""
    deadline = time.monotonic() + timeout
    alive = []
    for process in processes:
        try:
            process.wait(timeout=max(0.0, deadline - time.monotonic()))
        except psutil.TimeoutExpired:
            try:
                # A terminated grandchild may remain a zombie until its new parent reaps it.
                if process.status() != psutil.STATUS_ZOMBIE:
                    alive.append(process)
            except psutil.NoSuchProcess:
                pass
        except psutil.NoSuchProcess:
            pass
    return alive


class ViewerClient:
    """
    A connection to an instance of a Rerun viewer.

    Use the [`connect`][rerun.experimental.ViewerClient.connect] classmethod
    to attach to an already-running viewer, or
    [`spawn`][rerun.experimental.ViewerClient.spawn] to start a fresh one
    (e.g. in headless mode for CI screenshots).

    Spawned-viewer teardown:

    - Explicit [`quit`][rerun.experimental.ViewerClient.quit] always
      terminates the spawned viewer.
    - For an attached viewer (`detach_process=False`), exiting a `with` block
      or garbage-collecting the client also terminates the viewer.
    - A detached viewer keeps running through `with` exits and garbage
      collection. Only an explicit `quit()` shuts it down.

    Every method blocks until the viewer answers, for at most 15 seconds, after which it raises.
    `open` answers within 10 seconds, even if what it opens is still loading.

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
            that `quit()` can terminate it.
        _kill_on_exit:
            Internal — set by `spawn()` to indicate that implicit teardown
            (`__exit__`, `__del__`) should call `quit()`. See the class
            docstring for the full teardown rules.

        """
        from rerun_bindings import ViewerClientInternal

        # `quit()` kills the spawned viewer when `_pid` is set. Implicit
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
            [`quit`][rerun.experimental.ViewerClient.quit] explicitly.
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

    def close(
        self,
        urls: Literal["current", "all"] | str | Sequence[str] | None = None,
        *,
        store_ids: Sequence[StoreId] | None = None,
    ) -> list[StoreId]:
        """
        Close recordings in the viewer, and return the store ids of the ones that were closed.

        Closing by URL also stops loading from it.
        This only removes recordings from the viewer. Files on disk are untouched, and registered
        recordings stay in the catalog and can be reopened, but unsaved blueprint edits are lost.

        Raises if a recording is not open, or nothing is loaded from a URL. Then nothing is closed.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        urls:
            Close the recordings loaded from these URLs.
            `"current"` closes the active recording, and `"all"` every open one.
        store_ids:
            Close these recordings, as reported by [`get_state`][rerun.experimental.ViewerClient.get_state].

        """
        if (urls is None) == (store_ids is None):
            raise ValueError(
                "ViewerClient.close expects either `urls` or `store_ids`. "
                "To shut down a viewer started with `spawn`, use `quit()`."
            )
        target: dict[str, Any]
        if store_ids is not None:
            target = {"store_ids": {"store_ids": list(store_ids)}}
        elif isinstance(urls, str):
            target = {urls: True} if urls in ("current", "all") else {"urls": {"urls": [urls]}}
        else:
            target = {"urls": {"urls": list(urls or [])}}
        return list(self._viewer_control("close", target).get("closed", []))

    def open(self, urls: str | Sequence[str], *, navigate: bool = True) -> OpenResult:
        """
        Open URLs in the viewer, and wait about ten seconds at most for their initial load.

        A location the viewer already has open is not loaded again, only shown.
        For a recording on a server, or an `.rrd` registered with the viewer catalog, the initial
        load is the chunk index: the chunks themselves are fetched on demand afterwards.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        urls:
            Recording or blueprint files, `rerun://` dataset URIs, redap server or catalog URLs,
            or intra-recording links.
        navigate:
            Also show what it opens, making a recording the active one.
            If `False`, a recording loads in the background, and a server is connected to
            without being shown.

        """
        responses = [
            self._viewer_control("open", {"url": url, "navigate": navigate})
            for url in ([urls] if isinstance(urls, str) else urls)
        ]
        return OpenResult(
            store_ids=[store_id for response in responses for store_id in response.get("store_ids", [])],
            loading=[url for response in responses for url in response.get("loading", [])],
        )

    def save_screenshot(self, file_path: str, view_id: str | UUID | None = None) -> None:
        """
        Save a screenshot to a file.

        Blocks until the viewer has painted and saved it, for at most 15 seconds.

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
        self._viewer_control(
            "save_screenshot",
            _without_none(file_path=file_path, view_id=None if view_id is None else str(view_id)),
        )

    def get_location(self) -> str:
        """
        Report where the viewer is, such as a recording, a catalog entry, or the settings.

        Pass it to [`set_location`][rerun.experimental.ViewerClient.set_location] to come back to it later.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Raises
        ------
        RuntimeError
            If what the viewer shows has no location.

        """
        location: str = self._viewer_control("get_location", {}).get("location", "")
        return location

    def set_location(self, location: str) -> None:
        """
        Show a location the viewer already has open, like `ViewerState.location`.

        This only navigates, and never loads anything: use [`open`][rerun.experimental.ViewerClient.open] for that.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        location:
            A location, as [`get_state`][rerun.experimental.ViewerClient.get_state] reports it.

        Raises
        ------
        RuntimeError
            If the viewer does not have it open.

        """
        self._viewer_control("set_location", {"location": location})

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        sequence: int,
        play_state: Literal["paused", "playing"] | None = None,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        duration: int | float | timedelta | np.timedelta64,
        play_state: Literal["paused", "playing"] | None = None,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: str | None = None,
        *,
        timestamp: int | float | datetime | np.datetime64,
        play_state: Literal["paused", "playing"] | None = None,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: str,
        *,
        play_state: Literal["paused", "playing", "following"] | None = None,
        recording: StoreId | None = None,
    ) -> None: ...

    @overload
    def set_time(
        self,
        timeline: None = None,
        *,
        play_state: Literal["paused", "playing", "following"],
        recording: StoreId | None = None,
    ) -> None: ...

    def set_time(
        self,
        timeline: str | None = None,
        *,
        sequence: int | None = None,
        duration: int | float | timedelta | np.timedelta64 | None = None,
        timestamp: int | float | datetime | np.datetime64 | None = None,
        play_state: Literal["paused", "playing", "following"] | None = None,
        recording: StoreId | None = None,
    ) -> None:
        """
        Set the viewer's time cursor, active timeline, play state, or several of these at once.

        Parameters
        ----------
        timeline:
            The timeline to seek on, which becomes the active one.
            If omitted, the viewer uses its active timeline.
        sequence:
            A sequence index.
        duration:
            A duration in seconds, or a duration value with nanosecond precision.
        timestamp:
            Seconds since Unix epoch, or a timestamp value with nanosecond precision.
        play_state:
            `"paused"`, `"playing"`, or `"following"` the latest data.
            If omitted, the play state is left as it is.
            Following cannot be combined with a time.
        recording:
            The recording to seek, as reported by `get_state()`.
            If omitted, the viewer uses its active recording.

        """
        times_given = sum(value is not None for value in (sequence, duration, timestamp))
        if 1 < times_given:
            raise ValueError("ViewerClient.set_time expects at most one of sequence, duration, or timestamp")
        if timeline is None and times_given == 0 and play_state is None:
            raise ValueError("ViewerClient.set_time expects a timeline, sequence, duration, timestamp, or play_state")
        if times_given == 1 and play_state == "following":
            raise ValueError("ViewerClient.set_time cannot follow the latest data and also set a time")

        time: int | None
        if sequence is not None:
            time = sequence
        elif duration is not None:
            time = to_nanos(duration)
        elif timestamp is not None:
            time = to_nanos_since_epoch(timestamp)
        else:
            time = None

        self._viewer_control(
            "set_time_cursor",
            _without_none(
                store_id=recording,
                timeline=None if timeline is None else {"name": timeline},
                time=None if time is None else {"time": int(time)},
                play_state=None if play_state is None else f"PLAY_STATE_{play_state.upper()}",
            ),
        )

    def get_logs(self, after_sequence: int | None = None) -> list[LogEntry]:
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
        raw = self._viewer_control("get_logs", _without_none(after_sequence=after_sequence))
        return [
            LogEntry(
                sequence=entry.get("sequence", 0),
                level=entry.get("level", ""),
                target=entry.get("target", ""),
                message=entry.get("message", ""),
            )
            for entry in raw.get("entries", [])
        ]

    def get_state(self) -> ViewerState:
        """
        Report what the viewer is currently showing.

        Call this to learn which recording and timeline to drive, and which time values are valid,
        before moving the time cursor. A view's reports say what failed to visualize.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        """
        return _viewer_state_from_json(self._viewer_control("get_state", {}))

    def take_screenshot(self, view_id: str | UUID | None = None, *, max_size: int | None = None) -> bytes:
        """
        Take a screenshot of the viewer, or of one of its views, and return it as a PNG.

        Blocks until the viewer has painted it, for at most 15 seconds.

        Unlike [`save_screenshot`][rerun.experimental.ViewerClient.save_screenshot], nothing is
        written on the machine running the viewer.

        Parameters
        ----------
        view_id:
            The view to capture, as reported by `get_state()`. If omitted, the whole viewer.
        max_size:
            Longest side of the image, in pixels. A larger screenshot is scaled down to fit.
            If omitted, full resolution.

        """
        response = self._viewer_control(
            "save_screenshot",
            _without_none(view_id=None if view_id is None else str(view_id), max_size=max_size),
        )
        return base64.b64decode(response.get("png", ""))

    def get_recording_schema(
        self,
        store_id: StoreId | None = None,
        *,
        entity_path: str | None = None,
        max_entities: int | None = None,
        paths_only: bool | None = None,
    ) -> RecordingSchema:
        """
        Describe a recording: its timelines, its entities, and the components logged on them.

        This is the schema, not the data: a component listed here has been logged at some point.
        Use `paths_only` to see the shape of a large recording, then `entity_path` for the
        components of the entities that matter.

        Parameters
        ----------
        store_id:
            The recording, as reported by `get_state()`. If omitted, the active recording.
        entity_path:
            Only describe this entity and the entities below it, e.g. `/world/robot`.
        max_entities:
            How many entities to describe at most. Defaults to 100, or to 10000 with `paths_only`.
        paths_only:
            Name the entities without describing what is logged on them.

        """
        response = self._viewer_control(
            "get_recording_schema",
            _without_none(store_id=store_id) | _schema_request(entity_path, max_entities, paths_only),
        )
        return RecordingSchema(
            store_id=response.get("store_id", ""),
            timelines=[_timeline_from_json(timeline) for timeline in response.get("timelines", [])],
            entities=_entities_from_json(response.get("entities", [])),
            omitted_entities=response.get("omitted_entities", 0),
        )

    def get_dataset_schema(
        self,
        dataset_url: str | None = None,
        *,
        entity_path: str | None = None,
        max_entities: int | None = None,
        paths_only: bool | None = None,
    ) -> DatasetSchema:
        """
        Describe a dataset on a server: the entities of its segments, and the components logged on them.

        Nothing is opened or loaded.

        Parameters
        ----------
        dataset_url:
            A dataset `url` from the servers in `get_state()`. If omitted, the dataset on screen.
        entity_path:
            Only describe this entity and the entities below it, e.g. `/world/robot`.
        max_entities:
            How many entities to describe at most. Defaults to 100, or to 10000 with `paths_only`.
        paths_only:
            Name the entities without describing what is logged on them.
            Defaults to true without an `entity_path`.

        """
        response = self._viewer_control(
            "get_dataset_schema",
            _without_none(dataset_url=dataset_url) | _schema_request(entity_path, max_entities, paths_only),
        )
        return DatasetSchema(
            timelines=response.get("timelines", []),
            entities=_entities_from_json(response.get("entities", [])),
            omitted_entities=response.get("omitted_entities", 0),
            asset_ids=response.get("asset_ids", []),
        )

    def get_dataset_assets_schema(
        self,
        dataset_url: str | None = None,
        *,
        entity_path: str | None = None,
        max_entities: int | None = None,
        paths_only: bool | None = None,
    ) -> list[DatasetAssetSchema]:
        """
        Describe the assets of a dataset on a server, and the entities logged in each.

        An asset holds static data shared by the dataset's segments, such as a robot's meshes.

        Parameters
        ----------
        dataset_url:
            A dataset `url` from the servers in `get_state()`. If omitted, the dataset on screen.
        entity_path:
            Only describe this entity and the entities below it. Assets with nothing there are left out.
        max_entities:
            How many entities to describe at most, per asset.
        paths_only:
            Name the entities without describing what is logged on them.

        """
        response = self._viewer_control(
            "get_dataset_assets_schema",
            _without_none(dataset_url=dataset_url) | _schema_request(entity_path, max_entities, paths_only),
        )
        return [
            DatasetAssetSchema(
                asset_id=asset.get("asset_id", ""),
                entities=_entities_from_json(asset.get("entities", [])),
                omitted_entities=asset.get("omitted_entities", 0),
            )
            for asset in response.get("assets", [])
        ]

    def get_table_schema(self, table_url: str | None = None) -> list[TableColumnSchema]:
        """
        A table's columns in display order, hidden ones included.

        Parameters
        ----------
        table_url:
            A dataset or table `url` from the servers in `get_state()`, or a server's origin
            for its list of entries. If omitted, the table on screen.

        """
        response = self._viewer_control("get_table_schema", _without_none(table_url=table_url))
        return [
            TableColumnSchema(
                name=column.get("name", ""),
                display_name=column.get("display_name", ""),
                datatype=column.get("datatype", ""),
            )
            for column in response.get("columns", [])
        ]

    def get_blueprint(
        self,
        of: Literal["recording", "table"] | None = None,
        *,
        store_id: StoreId | None = None,
        table_url: str | None = None,
    ) -> Blueprint:
        """
        Read a whole blueprint.

        Blocks for up to 2 seconds while a recording or table on screen gets its default blueprint.

        A recording's blueprint has its layout, views, their properties, and overrides.
        A table's blueprint has its row filters, layout, and column settings.
        Without any arguments, reads the blueprint of what is on screen.
        See [Blueprints](https://rerun.io/docs/concepts/visualization/blueprints).

        Parameters
        ----------
        of:
            Whether to read a recording's or a table's blueprint.
            Implied by `store_id` or `table_url`.
        store_id:
            The recording. If omitted, the active recording.
        table_url:
            A dataset or table `url` from the servers in `get_state()`. If omitted, the table on screen.

        """
        response = self._viewer_control("get_blueprint", _blueprint_target(of, store_id, table_url))
        return Blueprint(
            blueprint_id=response.get("blueprint_id", ""),
            store_id=response.get("store_id"),
            table_url=response.get("table_url"),
            blueprint=json.loads(response.get("json", "{}")),
        )

    def set_blueprint(
        self,
        blueprint: dict[str, Any],
        of: Literal["recording", "table"] | None = None,
        *,
        store_id: StoreId | None = None,
        table_url: str | None = None,
    ) -> BlueprintInfo:
        """
        Replace a whole blueprint, in the form [`get_blueprint`][rerun.experimental.ViewerClient.get_blueprint] reads it.

        Anything the blueprint leaves out is cleared, so read it first, and change what you need.

        Parameters
        ----------
        blueprint:
            The blueprint, keyed by blueprint entity path, then archetype, then field.
            See [Blueprints](https://rerun.io/docs/concepts/visualization/blueprints).
        of:
            Whether to write a recording's or a table's blueprint.
            Implied by `store_id` or `table_url`.
        store_id:
            The recording. If omitted, the active recording.
        table_url:
            A dataset or table `url` from the servers in `get_state()`. If omitted, the table on screen.

        """
        target = _blueprint_target(of, store_id, table_url)
        if not target:
            raise ValueError("ViewerClient.set_blueprint expects `of`, `store_id`, or `table_url`")
        response = self._viewer_control("set_blueprint", {"json": json.dumps(blueprint)} | target)
        return BlueprintInfo(
            blueprint_id=response.get("blueprint_id", ""),
            store_id=response.get("store_id"),
            table_url=response.get("table_url"),
        )

    def get_transform_graph(
        self,
        store_id: StoreId | None = None,
        *,
        frame_filter: Literal["entity_path", "named", "all"] | None = None,
        edge_filter: Literal["static", "temporal", "all"] | None = None,
        include_transforms: bool | None = None,
    ) -> dict[str, Any]:
        """
        A recording's transform graph at its time cursor: its frames, and the edges between them.

        See [Transforms & Coordinate Frames](https://rerun.io/docs/concepts/logging-and-ingestion/transforms).

        Parameters
        ----------
        store_id:
            The recording. If omitted, the active recording.
        frame_filter:
            Frames derived from entity paths, explicitly named frames, or both. Defaults to `all`.
        edge_filter:
            Static edges, temporal edges, or both. Defaults to `all`.
        include_transforms:
            Include affine matrices and pinhole intrinsics, resolutions, and camera axes.

        """
        response = self._viewer_control(
            "get_transform_graph",
            _without_none(
                store_id=store_id,
                frame_filter=None if frame_filter is None else f"FRAME_FILTER_{frame_filter.upper()}",
                edge_filter=None if edge_filter is None else f"EDGE_FILTER_{edge_filter.upper()}",
                include_transforms=include_transforms,
            ),
        )
        graph: dict[str, Any] = json.loads(response.get("json", "{}"))
        return graph

    def highlight_rect(self, rect: tuple[float, float, float, float] | None, label: str | None = None) -> None:
        """
        Draw a pulsing outline around a rectangle, to point the user at something.

        It pulses until the user clicks anywhere.

        Parameters
        ----------
        rect:
            `(x, y, width, height)`, in logical points from the top-left corner of the viewer.
            `None` dismisses the current highlight.
        label:
            Short text shown beside the outline.

        """
        request = _without_none(label=label)
        if rect is not None:
            x, y, w, h = rect
            request["rect"] = {"x": x, "y": y, "w": w, "h": h}
        self._viewer_control("highlight_rect", request)

    def list_commands(
        self,
        ids: Sequence[str] | None = None,
        *,
        include_unavailable: bool | None = None,
        store_id: StoreId | None = None,
        server_origin: str | None = None,
    ) -> list[Command]:
        """
        The viewer's command palette commands, sorted by id, or the ones named in `ids`, in that order.

        Parameters
        ----------
        ids:
            Only these commands. If omitted, every command.
        include_unavailable:
            Include commands that cannot run right now. Commands named in `ids` are always included.
        store_id:
            Check `recording` commands against this recording instead of the active one.
        server_origin:
            Check `server` commands against this server instead of the selected one.

        """
        target = _without_none(store_id=store_id, server_origin=server_origin)
        if ids is None:
            listed = self._viewer_control(
                "list_commands", _without_none(include_unavailable=include_unavailable) | target
            )
            ids = [command.get("id", "") for command in listed.get("commands", [])]
        response = self._viewer_control("describe_commands", {"ids": list(ids)} | target)
        return [_command_from_json(command) for command in response.get("commands", [])]

    def run_command(
        self,
        id: str,
        *,
        store_id: StoreId | None = None,
        server_origin: str | None = None,
        allow_blocking_native_modal: bool | None = None,
    ) -> Command:
        """
        Run a command palette command by id, and return the command.

        Returns once the command is queued, not when it is done.
        Ask the user before running a `destructive` command.

        Parameters
        ----------
        id:
            Command id, as `list_commands()` reports it.
        store_id:
            The recording for a `recording` command. If omitted, the active one.
        server_origin:
            The server for a `server` command. If omitted, the selected one.
        allow_blocking_native_modal:
            Allow a command that opens a native modal, which blocks the viewer until a person closes it.

        """
        response = self._viewer_control(
            "run_command",
            {"id": id}
            | _without_none(
                store_id=store_id,
                server_origin=server_origin,
                allow_blocking_native_modal=allow_blocking_native_modal,
            ),
        )
        return _command_from_json(response.get("command", {}))

    def capture_profile_trace(self, file_path: str, *, num_frames: int | None = None) -> ProfileTrace:
        """
        Record a puffin profile of the next frames the viewer paints, and write it to a `.puffin` file.

        This is for profiling the viewer's own performance: where its time goes in each frame.
        Open the file in `puffin_viewer`, see [puffin](https://github.com/EmbarkStudios/puffin).

        Blocks until those frames are painted and the file is written, for at most 15 seconds.

        The file is written by the viewer process, on the machine running the viewer.
        The web viewer cannot capture profiles.

        Parameters
        ----------
        file_path:
            Where to write the file, relative to the viewer process's working directory.
        num_frames:
            How many frames to record, between 1 and 300. Defaults to 5.

        """
        response = self._viewer_control(
            "capture_profile_trace",
            _without_none(file_path=file_path, num_frames=num_frames),
        )
        return ProfileTrace(
            num_frames=response.get("num_frames", 0),
            file_path=response.get("file_path", ""),
        )

    def _viewer_control(self, operation: str, request: dict[str, Any]) -> dict[str, Any]:
        """Send one viewer-control operation, and return its response, both as canonical protobuf JSON."""
        response = json.loads(self._internal.viewer_control(json.dumps({operation: request})))
        if operation not in response:
            raise RuntimeError(f"Expected a {operation} response, got {response}")
        result: dict[str, Any] = response[operation]
        return result

    @deprecated("Use `open` instead.")
    def open_url(self, url: str, *, navigate: bool = True) -> bool:
        """Deprecated: use [`open`][rerun.experimental.ViewerClient.open] instead."""
        return not self.open(url, navigate=navigate).loading

    @deprecated("Use `close` instead.")
    def close_recordings(self, target: str | Sequence[StoreId] = "current") -> list[StoreId]:
        """Deprecated: use [`close`][rerun.experimental.ViewerClient.close] instead."""
        if target in ("current", "all"):
            return self.close(target)
        return self.close(store_ids=[target] if isinstance(target, str) else target)

    @deprecated("Use `get_state` instead.")
    def viewer_state(self) -> ViewerState:
        """Deprecated: use [`get_state`][rerun.experimental.ViewerClient.get_state] instead."""
        return self.get_state()

    @deprecated("Use `get_logs` instead.")
    def viewer_logs(self, after_sequence: int | None = None) -> list[LogEntry]:
        """Deprecated: use [`get_logs`][rerun.experimental.ViewerClient.get_logs] instead."""
        return self.get_logs(after_sequence)

    def quit(self, *, wait: bool = True, timeout: float = 5.0) -> None:
        """
        Quit the spawned viewer, waiting for it and its launcher to exit.

        Wait up to `timeout` seconds for termination, then force-kill any remaining
        processes and wait up to `timeout` seconds again.
        Raise `RuntimeError` if teardown fails.

        Emits a `UserWarning` and is a no-op if there is no spawned viewer to
        terminate (either the client never spawned one, or it has already
        been closed). Safe to call multiple times — only the first call has
        an effect.

        Parameters
        ----------
        wait:
            Wait for shutdown to finish. If `False`, only signal termination:
            do not wait for process exit or escalate to forced termination.
            The viewer's port may still be occupied when this method returns.
        timeout:
            Maximum seconds for each shutdown wait, not the total duration.
            Must be finite and greater than zero.
            On Windows, this also limits the `taskkill` command.

        """
        if not math.isfinite(timeout) or timeout <= 0:
            raise ValueError("timeout must be finite and greater than zero")

        pid = self._pid
        if pid is None:
            warnings.warn(
                "ViewerClient.quit() called with no viewer to terminate "
                "(the client was constructed via ViewerClient.connect(), or quit() was already called).",
                UserWarning,
                stacklevel=2,
            )
            return

        processes = []
        if wait:
            try:
                launcher = psutil.Process(pid)
            except psutil.NoSuchProcess:
                self._pid = None
                return

            # Capture descendants before signaling the launcher, while parentage is still intact.
            processes = [*launcher.children(recursive=True), launcher]
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
                    timeout=timeout,
                )
            else:
                # On unix the shim is launched in its own process group (see `spawn.rs`), and the
                # viewer child inherits it, so we can kill both cleanly with a single `killpg`.
                try:
                    os.killpg(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass

            if wait:
                alive = _wait_for_processes(processes, timeout=timeout)
                for process in alive:
                    try:
                        process.kill()
                    except psutil.NoSuchProcess:
                        pass
                alive = _wait_for_processes(alive, timeout=timeout)
                if alive:
                    raise RuntimeError(f"Viewer processes did not exit: {[process.pid for process in alive]}")
        except (OSError, psutil.Error, subprocess.SubprocessError) as err:
            raise RuntimeError(f"Failed to quit viewer process {pid}: {err}") from err
        self._pid = None

    def __enter__(self) -> ViewerClient:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        # Only attached viewers are torn down on `with` exit. Skip when
        # there's nothing to kill so we don't trip quit()'s warning if the
        # user already closed manually inside the block.
        if self._kill_on_exit and self._pid is not None:
            self.quit()

    def __del__(self) -> None:
        # Try stopping the viewer if it wasn't detached. Skip when there's
        # nothing to kill — both because there's no work to do and to avoid
        # tripping quit()'s warning during GC.
        try:
            if not getattr(self, "_kill_on_exit", False):
                return
            if getattr(self, "_pid", None) is None:
                return
            self.quit()
        except Exception:
            pass
