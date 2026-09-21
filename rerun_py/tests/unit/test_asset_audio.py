from __future__ import annotations

import pathlib

import numpy as np
import rerun as rr

ASSETS_DIR = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "audio"
WAV_FILEPATH = ASSETS_DIR / "sine_440hz_2s.wav"
AAC_FILEPATH = ASSETS_DIR / "sine_440hz_2s.aac"
assert WAV_FILEPATH.is_file()
assert AAC_FILEPATH.is_file()


def test_asset_audio_wav() -> None:
    blob_bytes = WAV_FILEPATH.read_bytes()
    blob_comp = rr.components.Blob(blob_bytes)

    rr.set_strict_mode(True)

    assets = [
        rr.AssetAudio(path=WAV_FILEPATH),
        rr.AssetAudio(path=str(WAV_FILEPATH)),
        rr.AssetAudio(contents=blob_bytes, media_type=rr.components.MediaType.WAV),
        rr.AssetAudio(contents=np.frombuffer(blob_bytes, dtype=np.uint8), media_type=rr.components.MediaType.WAV),
        rr.AssetAudio(contents=blob_comp, media_type=rr.components.MediaType.WAV),
    ]

    for asset in assets:
        assert asset.blob is not None
        assert asset.blob.as_arrow_array() == rr.components.BlobBatch(blob_comp).as_arrow_array()
        assert asset.media_type == rr.components.MediaTypeBatch(rr.components.MediaType.WAV)


def test_asset_audio_aac_media_type_from_path() -> None:
    rr.set_strict_mode(True)

    asset = rr.AssetAudio(path=AAC_FILEPATH)
    assert asset.media_type == rr.components.MediaTypeBatch(rr.components.MediaType.AAC)


def test_asset_audio_media_type_omitted() -> None:
    rr.set_strict_mode(True)

    asset = rr.AssetAudio(contents=b"not really audio")
    assert asset.blob is not None
    assert asset.media_type is None
