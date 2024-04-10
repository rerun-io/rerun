"""
Helper functions for Rerun scripts.

These helper functions can be used to wire up common Rerun features to your script CLi arguments.

Example
-------
```python
import argparse
import rerun as rr

parser = argparse.ArgumentParser()
rr.script_add_args(parser)
args = parser.parse_args()
rr.script_setup(args, "rerun_example_application")
# … Run your logging code here …
rr.script_teardown(args)
```

"""

from __future__ import annotations

from argparse import ArgumentParser, Namespace
from uuid import UUID

import rerun as rr
from rerun.recording_stream import RecordingStream


def script_add_args(parser: ArgumentParser) -> None:
    """
    Add common Rerun script arguments to `parser`.

    Parameters
    ----------
    parser : ArgumentParser
        The parser to add arguments to.

    """
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument(
        "--connect",
        dest="connect",
        action="store_true",
        help="Connect to an external viewer",
    )
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "-o",
        "--stdout",
        dest="stdout",
        action="store_true",
        help="Log data to standard output, to be piped into a Rerun Viewer",
    )


def script_setup(
    args: Namespace,
    application_id: str,
    recording_id: str | UUID | None = None,
    default_blueprint: rr.blueprint.BlueprintLike | None = None,
) -> RecordingStream:
    """
    Run common Rerun script setup actions. Connect to the viewer if necessary.

    Parameters
    ----------
    args : Namespace
        The parsed arguments from `parser.parse_args()`.
    application_id : str
        The application ID to use for the viewer.
    recording_id : Optional[str]
        Set the recording ID that this process is logging to, as a UUIDv4.

        The default recording_id is based on `multiprocessing.current_process().authkey`
        which means that all processes spawned with `multiprocessing`
        will have the same default recording_id.

        If you are not using `multiprocessing` and still want several different Python
        processes to log to the same Rerun instance (and be part of the same recording),
        you will need to manually assign them all the same recording_id.
        Any random UUIDv4 will work, or copy the recording id for the parent process.
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.

    """
    rr.init(
        application_id=application_id,
        recording_id=recording_id,
        default_enabled=True,
        strict=True,
    )

    rec: RecordingStream = rr.get_global_data_recording()  # type: ignore[assignment]

    # NOTE: mypy thinks these methods don't exist because they're monkey-patched.
    if args.stdout:
        rec.stdout(default_blueprint=default_blueprint)  # type: ignore[attr-defined]
    elif args.serve:
        rec.serve(default_blueprint=default_blueprint)  # type: ignore[attr-defined]
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rec.connect(args.addr, default_blueprint=default_blueprint)  # type: ignore[attr-defined]
    elif args.save is not None:
        rec.save(args.save, default_blueprint=default_blueprint)  # type: ignore[attr-defined]
    elif not args.headless:
        rec.spawn(default_blueprint=default_blueprint)  # type: ignore[attr-defined]

    return rec


def script_teardown(args: Namespace) -> None:
    """
    Run common post-actions. Sleep if serving the web viewer.

    Parameters
    ----------
    args : Namespace
        The parsed arguments from `parser.parse_args()`.

    """
    if args.serve:
        import time

        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print("Ctrl-C received. Exiting.")
