from __future__ import annotations

import os


def _spawn_viewer(
    *,
    port: int = 9876,
    memory_limit: str = "75%",
    hide_welcome_screen: bool = False,
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
    hide_welcome_screen:
        Hide the normal Rerun welcome screen.

    """

    import rerun_bindings

    # Let the spawned rerun process know it's just an app
    new_env = os.environ.copy()
    # NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
    # what, thus spawning a viewer is pointless (and probably not intended).
    if os.environ.get("_RERUN_TEST_FORCE_SAVE") is not None:
        return
    new_env["RERUN_APP_ONLY"] = "true"

    rerun_bindings.spawn(port=port, memory_limit=memory_limit, hide_welcome_screen=hide_welcome_screen)
