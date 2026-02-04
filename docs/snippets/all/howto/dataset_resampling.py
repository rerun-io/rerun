"""Sample snippets highlighting common performance-related improvements"""

import tempfile
import rerun as rr
from datafusion import col
from pathlib import Path

TMP_FILE = tempfile.NamedTemporaryFile(suffix=".rrd")
RRD_PATH = TMP_FILE.name

# region: get_dataset
sample_dataset_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "dataset"
server = rr.server.Server(datasets={"dataset": sample_dataset_path})
# Using OSS server for demonstration but in practice replace with
# the URL of your cloud instance
CATALOG_URL = server.url()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="dataset")
# endregion: get_dataset

# region: view_index_ranges
(
    dataset.get_index_ranges()
    .select(
        "rerun_segment_id", "time_1:start", "time_1:end", "time_2:start", "time_2:end", "time_3:start", "time_3:end"
    )
    .sort("rerun_segment_id")
    .show()
)
# endregion: view_index_ranges

# region: original_data
time_index = "time_3"
columns_of_interest = [
    "rerun_segment_id",
    time_index,
    "/obj1:Points3D:positions",
    "/obj2:Points3D:positions",
    "/obj3:Points3D:positions",
]
(dataset.reader(index=time_index).select(*columns_of_interest).sort("rerun_segment_id", time_index).show())

# +----------------------------------+--------+--------------------------+--------------------------+--------------------------+
# | rerun_segment_id                 | time_3 | /obj1:Points3D:positions | /obj2:Points3D:positions | /obj3:Points3D:positions |
# +----------------------------------+--------+--------------------------+--------------------------+--------------------------+
# | 141a866deb2d49f69eb3215e8a404ffc | 1      | [[49.0, 0.0, 0.0]]       | [[44.0, 1.0, 0.0]]       | [[1.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 2      | [[27.0, 0.0, 0.0]]       | [[42.0, 1.0, 0.0]]       |                          |
# | 141a866deb2d49f69eb3215e8a404ffc | 3      | [[25.0, 0.0, 0.0]]       | [[30.0, 1.0, 0.0]]       | [[3.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 4      | [[38.0, 0.0, 0.0]]       | [[19.0, 1.0, 0.0]]       |                          |
# | 141a866deb2d49f69eb3215e8a404ffc | 5      | [[17.0, 0.0, 0.0]]       | [[5.0, 1.0, 0.0]]        | [[5.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 6      | [[2.0, 0.0, 0.0]]        | [[35.0, 1.0, 0.0]]       |                          |
# | 141a866deb2d49f69eb3215e8a404ffc | 7      | [[44.0, 0.0, 0.0]]       | [[4.0, 1.0, 0.0]]        | [[7.0, 2.0, 0.0]]        |
# endregion: original_data

# region: resampled_data
resample_column = "/obj3:Points3D:positions"
times_of_interest = (
    dataset.reader(index=time_index).filter(col(resample_column).is_not_null()).select("rerun_segment_id", time_index)
)

(
    dataset.reader(index=time_index, using_index_values=times_of_interest, fill_latest_at=True)
    .select(*columns_of_interest)
    .sort("rerun_segment_id", time_index)
    .show()
)

# +----------------------------------+--------+--------------------------+--------------------------+--------------------------+
# | rerun_segment_id                 | time_3 | /obj1:Points3D:positions | /obj2:Points3D:positions | /obj3:Points3D:positions |
# +----------------------------------+--------+--------------------------+--------------------------+--------------------------+
# | 141a866deb2d49f69eb3215e8a404ffc | 1      | [[49.0, 0.0, 0.0]]       | [[44.0, 1.0, 0.0]]       | [[1.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 3      | [[25.0, 0.0, 0.0]]       | [[30.0, 1.0, 0.0]]       | [[3.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 5      | [[17.0, 0.0, 0.0]]       | [[5.0, 1.0, 0.0]]        | [[5.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 7      | [[44.0, 0.0, 0.0]]       | [[4.0, 1.0, 0.0]]        | [[7.0, 2.0, 0.0]]        |
# | 141a866deb2d49f69eb3215e8a404ffc | 10     | [[12.0, 0.0, 0.0]]       | [[6.0, 1.0, 0.0]]        | [[10.0, 2.0, 0.0]]       |
# | 141a866deb2d49f69eb3215e8a404ffc | 12     | [[13.0, 0.0, 0.0]]       | [[17.0, 1.0, 0.0]]       | [[12.0, 2.0, 0.0]]       |
# | 141a866deb2d49f69eb3215e8a404ffc | 13     | [[20.0, 0.0, 0.0]]       | [[32.0, 1.0, 0.0]]       | [[13.0, 2.0, 0.0]]       |
# endregion: resampled_data
