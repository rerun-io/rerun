"""
Common definitions and frameworks for building runnable tools with Rerun.
"""

import argparse
import logging
import decorators
from decorators import FuncDecorator


class script(FuncDecorator):
    def decorate(self, func, *decorator_args, **decorator_kwargs):
        def decorator(*args, **kwargs):
            parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
            return func(*args, **kwargs)
        return decorator


def setup_common_arguments(parser: argparse.ArgumentParser) -> None:
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
    parser.add_argument(
        "--addr",
        type=str,
        default=None,
        help="Connect to this ip:port",
    )
    parser.add_argument(
        "--save",
        type=str,
        default=None,
        help="Save data to a .rrd file at this path",
    )
