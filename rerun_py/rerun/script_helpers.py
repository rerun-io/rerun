from argparse import ArgumentParser, Namespace
from time import sleep

import rerun as rr


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
    rr.init(application_id=application_id)

    if args.serve:
        rr.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()


def script_teardown(args: Namespace) -> None:
    """
    Run common post-actions. Sleep if serving the web viewer.

    Parameters
    ----------
    args : Namespace
        The parsed arguments from `parser.parse_args()`.
    """
    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rr.save(args.save)
