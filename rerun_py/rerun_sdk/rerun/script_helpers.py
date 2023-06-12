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
rr.script_setup(args, "my_application")
# ... Run your logging code here ...
rr.script_teardown(args)
```

"""
from __future__ import annotations

from argparse import ArgumentParser, Namespace

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


def script_setup(
    args: Namespace,
    application_id: str,
) -> RecordingStream:
    """
    Run common Rerun script setup actions. Connect to the viewer if necessary.

    Parameters
    ----------
    args : Namespace
        The parsed arguments from `parser.parse_args()`.
    application_id : str
        The application ID to use for the viewer.

    """
    rr.init(
        application_id=application_id,
        default_enabled=True,
        strict=True,
    )

    rec: RecordingStream = rr.get_global_data_recording()  # type: ignore[assignment]

    # NOTE: mypy thinks these methods don't exist because they're monkey-patched.
    if args.serve:
        rec.serve()  # type: ignore[attr-defined]
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rec.connect(args.addr)  # type: ignore[attr-defined]
    elif args.save is not None:
        rec.save(args.save)  # type: ignore[attr-defined]
    elif not args.headless:
        rec.spawn()  # type: ignore[attr-defined]

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
