# region: imports
from __future__ import annotations

import numpy as np
import rerun as rr
# endregion: imports

# ----------------------------------------------------------------------------------------------
# Load and prepare the data

# TODO find rrd and check in to repo
# region: launch_server
server = rr.server.Server(datasets={"tutorial": ["/Users/nick/Downloads/face_tracking.rrd"]})

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
print(df["/blendshapes/0/jawOpen:Scalars:scalars"][160:180])
# endregion: print_frames

# convert the "jawOpen" column to a flat list of floats
# region: explode_jaw
pd_df["jawOpen"] = pd_df["/blendshapes/0/jawOpen:Scalars:scalars"].explode().astype(float)
print(df["jawOpen"][160:180])
# endregion: explode_jaw

# ----------------------------------------------------------------------------------------------
# Analyze the data

# region: filter_jaw

# compute the mouth state
pd_df["jawOpenState"] = pd_df["jawOpen"] > 0.15

# endregion: filter_jaw

# ----------------------------------------------------------------------------------------------
# Log the data back to the viewer

# TODO: this is the only way to get the application_id but is deprecated
application_id = rr.dataframe.load_recording("/Users/nick/Downloads/face_tracking.rrd").application_id()

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

input("Press Enter to terminate the server...")
