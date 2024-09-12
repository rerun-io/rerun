# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/asset_video.fbs".

# You can extend this class by creating a "AssetVideoExt" class in "asset_video_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .asset_video_ext import AssetVideoExt

__all__ = ["AssetVideo"]


@define(str=False, repr=False, init=False)
class AssetVideo(AssetVideoExt, Archetype):
    """
    **Archetype**: A video binary.

    NOTE: Videos can only be viewed in the Rerun web viewer.
    Only MP4 containers with a limited number of codecs are currently supported, and not in all browsers.
    Follow <https://github.com/rerun-io/rerun/issues/7298> for updates on the native support.

    In order to display a video, you need to log a [`archetypes.VideoFrameReference`][rerun.archetypes.VideoFrameReference] for each frame.

    ⚠️ **This is an experimental API! It is not fully supported, and is likely to change significantly in future versions.**

    Example
    -------
    ### Video with explicit frames:
    ```python
    # TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

    import sys

    import rerun as rr
    import numpy as np

    if len(sys.argv) < 2:
        # TODO(#7354): Only mp4 is supported for now.
        print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
        sys.exit(1)

    rr.init("rerun_example_asset_video_manual_frames", spawn=True)

    # Log video asset which is referred to by frame references.
    rr.set_time_seconds("video_time", 0)  # Make sure it's available on the timeline used for the frame references.
    rr.log("video", rr.AssetVideo(path=sys.argv[1]))

    # Send frame references for every 0.1 seconds over a total of 10 seconds.
    # Naturally, this will result in a choppy playback and only makes sense if the video is 10 seconds or longer.
    # TODO(#7368): Point to example using `send_video_frames`.
    #
    # Use `send_columns` to send all frame references in a single call.
    times = np.arange(0.0, 10.0, 0.1)
    rr.send_columns(
        "video",
        times=[rr.TimeSecondsColumn("video_time", times)],
        components=[rr.VideoFrameReference.indicator(), rr.components.VideoTimestamp.seconds(times)],
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/1200w.png">
      <img src="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in asset_video_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            blob=None,  # type: ignore[arg-type]
            media_type=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> AssetVideo:
        """Produce an empty AssetVideo, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    blob: components.BlobBatch = field(
        metadata={"component": "required"},
        converter=components.BlobBatch._required,  # type: ignore[misc]
    )
    # The asset's bytes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    media_type: components.MediaTypeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MediaTypeBatch._optional,  # type: ignore[misc]
    )
    # The Media Type of the asset.
    #
    # Supported values:
    # * `video/mp4`
    #
    # If omitted, the viewer will try to guess from the data blob.
    # If it cannot guess, it won't be able to render the asset.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
