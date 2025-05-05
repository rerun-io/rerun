from pathlib import Path

import numpy as np
import rerun as rr

RRD_DIR = Path("/tmp/redap_rrd")
RRD_DIR.mkdir(exist_ok=True)


def create_rrds(number: int) -> list[Path]:
    N_POINTS = 100


    paths = []

    for i in range(number):
        frames = rr.TimeColumn("frames", sequence=range(i * N_POINTS, (i+1) * N_POINTS))
        
        rec_path = RRD_DIR / f"rrd{i}.rrd"
        rec_path.unlink(missing_ok=True)
        
        paths.append(rec_path)

        rec = rr.RecordingStream("rerun_example_redap_data", recording_id=f"rec_id_{i}")
        rec.save(rec_path)
        rec.send_recording_name(f"data_{i}")
        rec.send_columns("/data", [frames], rr.Scalars.columns(scalars=np.random.rand(N_POINTS)))
        rec.flush(blocking=True)


    return paths



DATASET_NAME = "rerun_example_redap_data"

c = rr.catalog.CatalogClient("rerun+http://localhost:51234")


try:
    dataset = c.get_dataset(DATASET_NAME)
    dataset.delete()
except:
    pass  # who cares


d = c.create_dataset(DATASET_NAME)

for path in create_rrds(10):
    print(f"register {path}")
    d.register(f"file://{path.absolute()}")

df = d.dataframe_query_view(
    index="frames",
    contents={"/data": "rerun.components.Scalar"},
).df()

df.show(1000)
