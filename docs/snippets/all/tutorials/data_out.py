from __future__ import annotations

import os
import tempfile

import numpy as np
import rerun as rr

# ----------------------------------------------------------------------------------------------
# Load and prepare the data

# Create a temporary directory to hold our recording
with tempfile.TemporaryDirectory() as tmpdir:
    rrd_path = os.path.join(tmpdir, "face_tracking.rrd")

    # For this example, we assume the recording already exists
    # In practice, you would either:
    # 1. Download it: curl 'https://app.rerun.io/version/latest/examples/face_tracking.rrd' -o face_tracking.rrd
    # 2. Or use an existing recording path

    # Start a server with the recording
    with rr.server.Server(datasets={"face_tracking": [rrd_path]}) as server:
        dataset = server.client().get_dataset("face_tracking")

        # Query the recording into a pandas dataframe
        df = dataset.filter_contents(["/blendshapes/0/jawOpen"]).reader(index="frame_nr").to_pandas()

        # convert the "jawOpen" column to a flat list of floats
        df["jawOpen"] = df["/blendshapes/0/jawOpen:Scalar"].explode().astype(float)

        # ----------------------------------------------------------------------------------------------
        # Analyze the data

        # compute the mouth state
        df["jawOpenState"] = df["jawOpen"] > 0.15

        # ----------------------------------------------------------------------------------------------
        # Log the data back to the viewer

        # Connect to the viewer
        recording = rr.recording.load_recording(rrd_path)
        rr.init(recording.application_id(), recording_id=recording.recording_id())
        rr.connect_grpc()

        # log the jaw open state signal as a scalar
        rr.send_columns(
            "/jaw_open_state",
            indexes=[rr.TimeColumn("frame_nr", sequence=df["frame_nr"])],
            columns=rr.Scalars.columns(scalars=df["jawOpenState"]),
        )

        # log a `Label` component to the face bounding box entity
        target_entity = "/video/detector/faces/0/bbox"
        rr.log(target_entity, rr.Boxes2D.from_fields(show_labels=True), static=True)
        rr.send_columns(
            target_entity,
            indexes=[rr.TimeColumn("frame_nr", sequence=df["frame_nr"])],
            columns=rr.Boxes2D.columns(labels=np.where(df["jawOpenState"], "OPEN", "CLOSE")),
        )
