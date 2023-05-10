import argparse
import time

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")
    parser.add_argument(
        "--init-logging", action="store_true", help="'Accidentally initialize the logging stream as well'"
    )
    parser.add_argument("--append", action="store_true", help="Append to the blueprint instead of overwriting it")
    parser.add_argument("--auto-space-views", action="store_true", help="Automatically add space views based on heuristics")

    args = parser.parse_args()

    rr.init("Space", init_logging=args.init_logging, add_to_app_default_blueprint=args.append, spawn=True)
    rr.connect()

    if args.auto_space_views:
        rr.set_auto_space_views(True)

    rr.set_panel("blueprint_panel", expanded=False)
    rr.set_panel("selection_panel", expanded=False)
    rr.set_panel("timeline_panel", expanded=False)
    rr.add_space_view("moon-centric", "transforms3d/sun/planet/moon", ["transforms3d/sun/planet", "transforms3d/sun/planet/moon"])


if __name__ == "__main__":
    main()
