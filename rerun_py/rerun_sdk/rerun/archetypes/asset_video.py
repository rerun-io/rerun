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

    Only MP4 containers with AV1 is generally supported,
    though the web viewer sometimes supports some more video codex, depending on browser.

    See <https://rerun.io/docs/reference/video?speculative-link> for details of what is and isn't supported.

    In order to display a video, you also need to log a [`archetypes.VideoFrameReference`][rerun.archetypes.VideoFrameReference] for each frame.

    Examples
    --------
    ### Video with automatically determined frames:
    ```python
    # TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

    import sys

    import rerun as rr

    if len(sys.argv) < 2:
        # TODO(#7354): Only mp4 is supported for now.
        print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
        sys.exit(1)

    rr.init("rerun_example_asset_video_auto_frames", spawn=True)

    # Log video asset which is referred to by frame references.
    video_asset = rr.AssetVideo(path=sys.argv[1])
    rr.log("video", video_asset, static=True)

    # Send automatically determined video frame timestamps.
    frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
    rr.send_columns(
        "video",
        # Note timeline values don't have to be the same as the video timestamps.
        times=[rr.TimeNanosColumn("video_time", frame_timestamps_ns)],
        components=[rr.VideoFrameReference.indicator(), rr.components.VideoTimestamp.nanoseconds(frame_timestamps_ns)],
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

    ### Demonstrates manual use of video frame references:
    ```python
    # TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

    import sys

    import rerun as rr
    import rerun.blueprint as rrb

    if len(sys.argv) < 2:
        # TODO(#7354): Only mp4 is supported for now.
        print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
        sys.exit(1)

    rr.init("rerun_example_asset_video_manual_frames", spawn=True)

    # Log video asset which is referred to by frame references.
    rr.log("video_asset", rr.AssetVideo(path=sys.argv[1]), static=True)

    # Create two entities, showing the same video frozen at different times.
    rr.log(
        "frame_1s",
        rr.VideoFrameReference(
            timestamp=rr.components.VideoTimestamp(seconds=1.0),
            video_reference="video_asset",
        ),
    )
    rr.log(
        "frame_2s",
        rr.VideoFrameReference(
            timestamp=rr.components.VideoTimestamp(seconds=2.0),
            video_reference="video_asset",
        ),
    )

    # Send blueprint that shows two 2D views next to each other.
    rr.send_blueprint(rrb.Horizontal(rrb.Spatial2DView(origin="frame_1s"), rrb.Spatial2DView(origin="frame_2s")))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/1200w.png">
      <img src="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/full.png" width="640">
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
