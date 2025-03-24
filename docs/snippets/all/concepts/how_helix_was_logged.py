rr.set_time("stable_time", duration=time)

beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
rr.log(
    "helix/structure/scaffolding/beads",
    rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1))
)
