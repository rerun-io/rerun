"""
Helper functions for Rerun scripts.

These helper functions can be used to wire up common Rerun features to your script CLi arguments.

Example
-------
```python
import argparse
import depthai_viewer as viewer

parser = argparse.ArgumentParser()
viewer.script_add_args(parser)
args = parser.parse_args()
viewer.script_setup(args, "my_application")
# ... Run your logging code here ...
viewer.script_teardown(args)
```

"""
from argparse import ArgumentParser, Namespace

import depthai_viewer as viewer


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
) -> None:
    """
    Run common Rerun script setup actions. Connect to the viewer if necessary.

    Parameters
    ----------
    args : Namespace
        The parsed arguments from `parser.parse_args()`.
    application_id : str
        The application ID to use for the viewer.

    """
    viewer.init(application_id=application_id, default_enabled=True, strict=True)

    if args.serve:
        viewer.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        viewer.connect(args.addr)
    elif args.save is not None:
        viewer.save(args.save)
    elif not args.headless:
        viewer.spawn()


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
