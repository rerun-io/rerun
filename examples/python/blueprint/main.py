"""Example of using the blueprint APIs to configure Rerun."""

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.experimental as rr_exp


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")

    parser.add_argument("--blueprint-only", action="store_true", help="Only send the blueprint")
    parser.add_argument("--skip-blueprint", action="store_true", help="Don't send the blueprint")
    parser.add_argument(
        "--no-append-default", action="store_false", help="Append to the default blueprint instead of replacing it"
    )
    parser.add_argument("--auto-space-views", action="store_true", help="Automatically add space views")

    args = parser.parse_args()

    if args.blueprint_only:
        # If only using blueprint, it's important to specify init_logging=False
        rr.init(
            "Blueprint demo",
            init_logging=False,
            exp_init_blueprint=True,
            exp_add_to_app_default_blueprint=args.no_append_default,
            spawn=True,
        )
    else:
        rr.init(
            "Blueprint demo",
            exp_init_blueprint=True,
            exp_add_to_app_default_blueprint=args.no_append_default,
            spawn=True,
        )

    if not args.blueprint_only:
        img = np.zeros([128, 128, 3], dtype="uint8")
        for i in range(8):
            img[(i * 16) + 4 : (i * 16) + 12, :] = (0, 0, 200)
        rr.log_image("image", img)
        rr.log_rect("rect/0", [16, 16, 64, 64], label="Rect1", color=(255, 0, 0))
        rr.log_rect("rect/1", [48, 48, 64, 64], label="Rect2", color=(0, 255, 0))

    if not args.skip_blueprint:
        if args.auto_space_views:
            rr_exp.set_auto_space_views(True)

        rr_exp.set_panels(all_expanded=False)

        rr_exp.add_space_view(name="overlaid", origin="/", entity_paths=["image", "rect/0", "rect/1"])

    # Workaround https://github.com/rerun-io/rerun/issues/2124
    import time

    time.sleep(0.1)


if __name__ == "__main__":
    main()
