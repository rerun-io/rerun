from __future__ import annotations

import pathlib

import numpy as np
import rerun as rr

CUBE_FILE = pathlib.Path(__file__).parent.parent.parent.parent / "docs" / "assets" / "cube.glb"
assert CUBE_FILE.is_file()


def test_asset3d() -> None:
    blob_bytes = CUBE_FILE.read_bytes()
    blob_comp = rr.components.Blob(blob_bytes)

    all_input: list[tuple[rr.components.BlobLike | str | pathlib.Path, rr.components.MediaType | None]] = [
        (CUBE_FILE, None),
        (str(CUBE_FILE), None),
        (blob_bytes, rr.components.MediaType.GLB),
        (np.frombuffer(blob_bytes, dtype=np.uint8), rr.components.MediaType.GLB),
        (blob_comp, rr.components.MediaType.GLB),
    ]

    rr.set_strict_mode(True)

    assets = [rr.Asset3D(blob, media_type=typ) for blob, typ in all_input]

    for asset in assets:
        assert asset.blob.as_arrow_array() == rr.components.BlobBatch(blob_comp).as_arrow_array()
        assert asset.media_type == rr.components.MediaTypeBatch(rr.components.MediaType.GLB)
        assert asset.transform is None


def test_asset3d_transform() -> None:
    asset = rr.Asset3D(CUBE_FILE, transform=rr.datatypes.TranslationRotationScale3D(translation=[1, 2, 3]))

    assert asset.transform is not None
    assert (
        asset.transform.as_arrow_array()
        == rr.components.OutOfTreeTransform3DBatch(
            rr.datatypes.TranslationRotationScale3D(translation=[1, 2, 3])
        ).as_arrow_array()
    )
