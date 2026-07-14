from __future__ import annotations

import os
import signal
import subprocess
import warnings
from typing import TYPE_CHECKING

from rerun._arrow import to_record_batch

if TYPE_CHECKING:
    from types import TracebackType
    from uuid import UUID

    import datafusion
    import pyarrow as pa

    from rerun_bindings import ViewerClientInternal


_DEFAULT_URL = "rerun+http://127.0.0.1:9876/proxy"


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
