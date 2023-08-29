"""Log different transforms between three arrows."""
import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

rr.init("rerun_example_transform3d", spawn=True)

origin = [0, 0, 0]
base_vector = [0, 1, 0]

rr.log_arrow("base", origin=origin, vector=base_vector)

rr2.log("base/translated", rr2.Transform3D(rrd.TranslationRotationScale3D(translation=[1, 0, 0])))

rr.log_arrow("base/translated", origin=origin, vector=base_vector)

rr2.log(
    "base/rotated_scaled",
    rrd.TranslationRotationScale3D(
        rotation=rrd.RotationAxisAngle(axis=[0, 0, 1], radians=3.14 / 4), scale=rrd.Scale3D(2)
    ),
)

rr.log_arrow("base/rotated_scaled", origin=origin, vector=base_vector)
