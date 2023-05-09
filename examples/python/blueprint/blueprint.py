import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")
    parser.add_argument("--append", action="store_true", help="Append to the blueprint instead of overwriting it")
    parser.add_argument("--enable-heuristics", action="store_true", help="Enable heuristics for the blueprint")

    args = parser.parse_args()

    rr.init("Space", append_blueprint=args.append)
    rr.connect()

    if args.enable_heuristics:
        rr.enable_heuristics()

    rr.set_panel("blueprint_panel", expanded=False)
    rr.set_panel("selection_panel", expanded=False)
    rr.set_panel("timeline_panel", expanded=False)
    rr.add_space_view("earth-centric", "transforms3d/sun/planet", ["transforms3d/sun", "transforms3d/sun/planet"])


if __name__ == "__main__":
    main()
