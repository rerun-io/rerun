import os.path
import subprocess
from pathlib import Path

import rerun as rr
import pyarrow as pa

#  RERUN_ALLOW_MISSING_BIN=1 maturin develop --manifest-path ~/src/rerun/rerun_py/Cargo.toml --features remote

# cargo run -p re_viewer --bin thumbnail_generator --features re_viewer_context/testing --features re_viewport_blueprint/testing ~/2d.rrd thumb.png


# rrd_path = Path("/Users/lucas/Downloads/Archive/77abe62c-c767-4e32-b035-27b9fadd8588.rrd")
conn = rr.remote.connect("http://127.0.0.1:51234")

# for rrd_path in list(Path("/tmp/logs").glob("*.rrd"))[:3]:
for rrd_path in Path("/Users/lucas/Downloads/Archive").glob("*.rrd"):
    img_path = Path("thumb.png")

    subprocess.run("cargo run -p re_thumbnail_generator {} {}".format(rrd_path, img_path), shell=True)

    img = rr.EncodedImage(path=img_path)

    field = pa.field(
        "field1",
        img.blob.pa_array.type,
        metadata={
            "rerun.kind": "data",
            "rerun.component": "rerun.components.Blob",
            "rerun.archetype_field": "thumbnail",
        },
    )
    schema = pa.schema([field])

    try:
        conn.register(
            "default",
            rrd_path.absolute().as_uri(),
            metadata=pa.RecordBatch.from_arrays([img.blob.pa_array], schema=schema),
        )
    except Exception as e:
        print(e)

print("Done")
