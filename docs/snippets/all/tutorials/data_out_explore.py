from __future__ import annotations

import rerun as rr

# load the recording
recording = rr.dataframe.load_recording("face_tracking.rrd")

# query the recording into a pandas dataframe
record_batches = recording.view(index="frame_nr", contents="/blendshapes/0/jawOpen").select()
df = record_batches.read_pandas()

print(df)

df["jawOpen"] = df["/blendshapes/0/jawOpen:Scalar"].explode().astype(float)
print(df)

# df.rename(columns={"/blendshapes/0/jawOpen:Scalar": "jawOpen"}, inplace=True)


query = recording.view(index="frame_nr", contents="/blendshapes/0/jawOpen")
table = query.select().read_all()

df = table.to_pandas()

print(table)

df = query.select().read_pandas()
