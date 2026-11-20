"""Log the contents of CSV and HDF5 files, which have no built-in importer."""

import tempfile
from pathlib import Path

import pandas as pd
import pyarrow as pa

import rerun as rr

repo_root = Path(__file__).resolve().parents[4]
hdf5_path = (
    repo_root
    / "crates"
    / "data_flow"
    / "re_hdf5"
    / "tests"
    / "assets"
    / "h5py_compat.h5"
)

csv_path = Path(tempfile.mkdtemp()) / "data.csv"
csv_path.write_text("frame,x,y\n0,0.0,1.0\n1,0.5,1.5\n2,1.0,2.0\n")

rr.init("rerun_example_log_csv_and_hdf5")

# region: csv
df = pd.read_csv(csv_path)  # e.g. columns: frame, x, y
rr.send_dataframe(pa.Table.from_pandas(df), index="frame", entity_path="/data")
# endregion: csv

# region: hdf5
reader = rr.experimental.Hdf5Reader(hdf5_path)
# Rows are indexed on a generated `row_index` timeline.
rr.send_chunks(reader.stream())
# endregion: hdf5
