"""The DNA-abacus example from the Log and Ingest tutorial."""

# region: imports
from math import tau

import numpy as np

import rerun as rr
from rerun.utilities import bounce_lerp, build_color_spiral

# endregion: imports


def main() -> None:
    # region: init
    rr.init("rerun_example_dna_abacus", spawn=True)
    # endregion: init

    # The fix for the latest-at lesson — see "Latest-at semantics" in the tutorial.
    # region: latest_at_fix
    rr.set_time("stable_time", duration=0)
    # endregion: latest_at_fix

    NUM_POINTS = 100

    # region: first_points
    points1, colors1 = build_color_spiral(NUM_POINTS)
    points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau * 0.5)

    rr.log("dna/structure/left", rr.Points3D(points1, colors=colors1, radii=0.08))
    rr.log("dna/structure/right", rr.Points3D(points2, colors=colors2, radii=0.08))
    # endregion: first_points

    # region: scaffolding
    rr.log(
        "dna/structure/scaffolding",
        rr.LineStrips3D(np.stack((points1, points2), axis=1), colors=[128, 128, 128]),
    )
    # endregion: scaffolding

    # region: beads
    offsets = np.random.rand(NUM_POINTS)
    beads = [bounce_lerp(points1[n], points2[n], offsets[n]) for n in range(NUM_POINTS)]
    colors = [[int(bounce_lerp(80, 230, offsets[n] * 2))] for n in range(NUM_POINTS)]
    rr.log(
        "dna/structure/scaffolding/beads",
        rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1)),
    )
    # endregion: beads

    time_offsets = np.random.rand(NUM_POINTS)

    # region: time_loop
    for i in range(400):
        time = i * 0.01
        rr.set_time("stable_time", duration=time)

        times = np.repeat(time, NUM_POINTS) + time_offsets
        beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
        colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
        rr.log(
            "dna/structure/scaffolding/beads",
            rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1)),
        )
    # endregion: time_loop

    # region: transform_loop
    for i in range(400):
        time = i * 0.01
        rr.set_time("stable_time", duration=time)
        rr.log(
            "dna/structure",
            rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau)),
        )
    # endregion: transform_loop


if __name__ == "__main__":
    main()
