import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")
    parser.add_argument(
        "--init-logging", action="store_true", help="'Accidentally initialize the logging stream as well'"
    )
    parser.add_argument("--append", action="store_true", help="Append to the blueprint instead of overwriting it")

    args = parser.parse_args()

    rr.init("Space", init_blueprint=True, init_logging=args.init_logging, append_blueprint=args.append)
    rr.connect()

    rr.add_space_view("earth-centric", "transforms3d/sun/planet", ["transforms3d/sun", "transforms3d/sun/planet"])


if __name__ == "__main__":
    main()
