from __future__ import annotations

import rerun as rr
import numpy as np

# ----------------------------------------------------------------------------------------------
# Load and prepare the data

# load the recording
recording = rr.dataframe.load_recording("face_tracking.rrd")

# query the recording into a pandas dataframe
record_batches = recording.view(index="frame_nr", contents="/blendshapes/0/jawOpen").select()
df = record_batches.read_pandas()

# convert the "jawOpen" column to a flat list of floats
df["jawOpen"] = df["/blendshapes/0/jawOpen:Scalar"].explode().astype(float)

# ----------------------------------------------------------------------------------------------
# Analyze the data

# compute the mouth open state
df["mouth_open"] = df["jawOpen"] > 0.15

# find the state transitions
diff = np.diff(df["mouth_open"], prepend=df["mouth_open"].iloc[0])
open_mouth_frames = df["frame_nr"][diff == 1].values
closed_mouth_frames = df["frame_nr"][diff == -1].values

# add the initial state
if df["mouth_open"].iloc[0] == 1:
    open_mouth_frames = np.concatenate([[0], open_mouth_frames])
else:
    closed_mouth_frames = np.concatenate([[0], closed_mouth_frames])

# ----------------------------------------------------------------------------------------------
# Log the data back to the viewer


# Connect to the viewer
rr.init(recording.application_id(), recording_id=recording.recording_id())
rr.connect()

rr.log_components("/video/detector/faces/0/bbox", [rr.components.ShowLabels(True)], static=True)

rr.send_columns(
    "/video/detector/faces/0/bbox",
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.TextBatch(np.where(df["mouth_open"], "OPEN", "CLOSE")),
    ],
)

# for frame_nr in open_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log_components("/video/detector/faces/0/bbox", [rr.components.Text("OPEN")])
# for frame_nr in closed_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log_components("/video/detector/faces/0/bbox", [rr.components.Text("CLOSE")])

# # log state transitions as a red dot showing on top the video feed
# for frame_nr in open_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log("/mouth_open/indicator", rr.Points2D([100, 100], radii=20, colors=[255, 0, 0]))
# for frame_nr in closed_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log("/mouth_open/indicator", rr.Clear(recursive=False))
#
# # log state transitions to a TextLog view
# for frame_nr in open_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log("/mouth_open/state", rr.TextLog(f"mouth opened"))
# for frame_nr in closed_mouth_frames:
#     rr.set_time_sequence("frame_nr", frame_nr)
#     rr.log("/mouth_open/state", rr.TextLog(f"mouth closed"))

# log the mouth open signal as a scalar
rr.send_columns(
    "/mouth_open/values",
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.ScalarBatch(df["mouth_open"].values),
    ],
)
