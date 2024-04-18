from __future__ import annotations

import argparse
from pathlib import Path

from . import active_examples

PROJECT_ROOT = Path(__file__).parent.parent.parent.parent.parent


def cmd_list(args):
    examples = active_examples()

    for example in sorted(examples, key=lambda e: e.name):
        rel_path = example.path.relative_to(PROJECT_ROOT)

        # TODO(ab): add env marker when pixi supports them
        print(f'{example.name} = {{ path = "{rel_path}", editable = true }} ')


def main():
    parser = argparse.ArgumentParser(prog="all_examples", description="Meta-project to enumerate all Python example")
    subparsers = parser.add_subparsers(dest="command")

    # `list` command
    list_parser = subparsers.add_parser("list", help="List all examples")

    args = parser.parse_args()
    if args.command == "list":
        cmd_list(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
