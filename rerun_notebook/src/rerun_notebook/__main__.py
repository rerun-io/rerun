from __future__ import annotations

import argparse

from .asset_server import serve_assets


def main() -> None:
    parser = argparse.ArgumentParser()

    subparsers = parser.add_subparsers(dest="command", help="Which command to run")

    serve_parser = subparsers.add_parser("serve")
    serve_parser.add_argument("--bind-address", default="localhost")
    serve_parser.add_argument("--port", type=int, default=8080)

    args = parser.parse_args()

    if args.command == "serve":
        serve_assets(args.bind_address, args.port)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
