# region: imports
from __future__ import annotations

from pathlib import Path

import numpy as np
import rerun as rr

# endregion: imports

# ----------------------------------------------------------------------------------------------
# Load and prepare the data

repo_root = Path(__file__).parent.parent.parent.parent.parent
example_rrd = repo_root / "tests" / "assets" / "rrd" / "examples" / "face_tracking.rrd"
assert example_rrd.exists(), f"Example RRD not found at {example_rrd}"
# region: launch_server
server = rr.server.Server(datasets={"tutorial": [example_rrd]})

client = rr.catalog.CatalogClient(address=server.address())
# endregion: launch_server

# query the recording into a pandas dataframe
# region: query_data
dataset = client.get_dataset("tutorial")
df = dataset.filter_contents("/blendshapes/0/jawOpen").reader(index="frame_nr")
# endregion: query_data

# region: to_pandas
pd_df = df.to_pandas()
# endregion: to_pandas

# region: print_frames
print(pd_df["/blendshapes/0/jawOpen:Scalars:scalars"][160:180])
# endregion: print_frames

# convert the "jawOpen" column to a flat list of floats
print(pd_df)
# region: explode_jaw
pd_df["jawOpen"] = (
    pd_df["/blendshapes/0/jawOpen:Scalars:scalars"].explode().astype(float)
)
print(pd_df["jawOpen"][160:180])
# endregion: explode_jaw

# ----------------------------------------------------------------------------------------------
# Analyze the data

# region: filter_jaw

# compute the mouth state
pd_df["jawOpenState"] = pd_df["jawOpen"] > 0.15

# endregion: filter_jaw

# ----------------------------------------------------------------------------------------------
# Log the data back to the viewer

application_id = rr.recording.load_recording(example_rrd).application_id()

# Connect to the viewer
# region: connect_viewer
rr.init(application_id, recording_id=dataset.segment_ids()[0])
rr.connect_grpc()
# endregion: connect_viewer

# log the jaw open state signal as a scalar
# region: send_columns
rr.send_columns(
    "/jaw_open_state",
    indexes=[rr.TimeColumn("frame_nr", sequence=pd_df["frame_nr"])],
    columns=rr.Scalars.columns(scalars=pd_df["jawOpenState"]),
)
# endregion: send_columns

# log a `Label` component to the face bounding box entity
# region: log_labels
target_entity = "/video/detector/faces/0/bbox"
rr.log(target_entity, rr.Boxes2D.from_fields(show_labels=True), static=True)
rr.send_columns(
    target_entity,
    indexes=[rr.TimeColumn("frame_nr", sequence=pd_df["frame_nr"])],
    columns=rr.Boxes2D.columns(labels=np.where(pd_df["jawOpenState"], "OPEN", "CLOSE")),
)
# endregion: log_labels
