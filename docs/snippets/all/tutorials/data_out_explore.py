from __future__ import annotations

import rerun as rr

# Load the recording
recording = rr.dataframe.load_recording("face_tracking.rrd")

# This line is pasted from the dataframe view "copy-as-code" feature
df = (
    recording.view(index="frame_nr", contents="/blendshapes/0/**")
    .select(
        rr.dataframe.IndexColumnSelector("frame_nr"),
        rr.dataframe.IndexColumnSelector("frame_time"),
        # TODO(#7629): should be able to use
        rr.dataframe.ComponentColumnSelector(
            "/blendshapes/0/jawOpen",
            "rerun.components.Scalar",
        ),
    )
    .read_pandas()
)

print(df)

#      frame_nr              frame_time /blendshapes/0/jawOpen:Scalar
# 0           0 1970-01-01 00:00:00.000      [0.00025173681206069887]
# 1           2 1970-01-01 00:00:00.080        [0.006588249001652002]
# 2           4 1970-01-01 00:00:00.160       [0.0010859279427677393]
# 3           6 1970-01-01 00:00:00.240       [0.0009152492275461555]
# 4           8 1970-01-01 00:00:00.320       [0.0013251719065010548]
# ..        ...                     ...                           ...
# 663       777 1970-01-01 00:00:31.080        [0.010198011063039303]
# 664       778 1970-01-01 00:00:31.120        [0.011381848715245724]
# 665       779 1970-01-01 00:00:31.160        [0.011795849539339542]
# 666       780 1970-01-01 00:00:31.200           [0.013143265619874]
# 667       781 1970-01-01 00:00:31.240         [0.01528632827103138]
#
# [668 rows x 3 columns]
