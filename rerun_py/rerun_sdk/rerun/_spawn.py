from __future__ import annotations

import socket


# TODO(#4019): application-level handshake
def _check_for_existing_viewer(port: int) -> bool:
    try:
        # Try opening a connection to the port to see if something is there
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(1)
        s.connect(("127.0.0.1", port))
        return True
    except Exception:
        # If the connection times out or is refused, the port is not open
        return False
    finally:
        # Always close the socket to release resources
        s.close()


def _spawn_viewer(
    *,
    port: int = 9876,
    memory_limit: str = "75%",
) -> None:
    """
    Internal helper to spawn a Rerun Viewer, listening on the given port.

    Blocks until the viewer is ready to accept connections.

    Used by [rerun.spawn][]

    Parameters
    ----------
    port:
        The port to listen on.
    memory_limit:
        An upper limit on how much memory the Rerun Viewer should use.
        When this limit is reached, Rerun will drop the oldest data.
        Example: `16GB` or `50%` (of system total).

    """

    import os
    from time import sleep

    import rerun_bindings

    # Let the spawned rerun process know it's just an app
    new_env = os.environ.copy()
    # NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
    # what, thus spawning a viewer is pointless (and probably not intended).
    if os.environ.get("_RERUN_TEST_FORCE_SAVE") is not None:
        return
    new_env["RERUN_APP_ONLY"] = "true"

    # TODO(jleibs): More options to opt out of this behavior.
    if _check_for_existing_viewer(port):
        # Using print here for now rather than `logging.info` because logging.info isn't
        # visible by default.
        #
        # If we spawn a process it's going to send a bunch of stuff to stdout anyways.
        print(f"Found existing process on port {port}. Trying to connect.")
    else:
        # start_new_session=True ensures the spawned process does NOT die when
        # we hit ctrl-c in the terminal running the parent Python process.
        rerun_bindings.spawn(port=port, memory_limit=memory_limit)

        # Give the newly spawned Rerun Viewer some time to bind.
        #
        # NOTE: The timeout only covers the TCP handshake: if no process is bound to that address
        # at all, the connection will fail immediately, irrelevant of the timeout configuration.
        # For that reason we use an extra loop.
        for _ in range(0, 5):
            _check_for_existing_viewer(port)
            sleep(0.1)
