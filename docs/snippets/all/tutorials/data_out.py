from __future__ import annotations

import numpy as np
import rerun as rr

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

# compute the mouth state
df["jawOpenState"] = df["jawOpen"] > 0.15

# ----------------------------------------------------------------------------------------------
# Log the data back to the viewer

# Connect to the viewer
rr.init(recording.application_id(), recording_id=recording.recording_id())
rr.connect()

# log the jaw open state signal as a scalar
rr.send_columns(
    "/jaw_open_state",
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.ScalarBatch(df["jawOpenState"]),
    ],
)

# log a `Label` component to the face bounding box entity
target_entity = "/video/detector/faces/0/bbox"
rr.log_components(target_entity, [rr.components.ShowLabels(True)], static=True)
rr.send_columns(
    target_entity,
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.TextBatch(np.where(df["jawOpenState"], "OPEN", "CLOSE")),
    ],
)
