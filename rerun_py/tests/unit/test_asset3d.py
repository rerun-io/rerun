from __future__ import annotations

import pathlib

import numpy as np
import rerun as rr

CUBE_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "mesh" / "cube.glb"
assert CUBE_FILEPATH.is_file()


def test_asset3d() -> None:
    blob_bytes = CUBE_FILEPATH.read_bytes()
    blob_comp = rr.components.Blob(blob_bytes)

    rr.set_strict_mode(True)

    assets = [
        rr.Asset3D(path=CUBE_FILEPATH),
        rr.Asset3D(path=str(CUBE_FILEPATH)),
        rr.Asset3D(contents=blob_bytes, media_type=rr.components.MediaType.GLB),
        rr.Asset3D(contents=np.frombuffer(blob_bytes, dtype=np.uint8), media_type=rr.components.MediaType.GLB),
        rr.Asset3D(contents=blob_comp, media_type=rr.components.MediaType.GLB),
    ]

    for asset in assets:
        assert asset.blob is not None
        assert asset.blob.as_arrow_array() == rr.components.BlobBatch(blob_comp).as_arrow_array()
        assert asset.media_type == rr.components.MediaTypeBatch(rr.components.MediaType.GLB)
