from __future__ import annotations

import pathlib
from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt
import rerun_bindings as bindings
from typing_extensions import deprecated

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import datatypes


class AssetVideoExt:
    """Extension for [AssetVideo][rerun.archetypes.AssetVideo]."""

    def __init__(
        self: Any,
        *,
        path: str | pathlib.Path | None = None,
        contents: datatypes.BlobLike | None = None,
        media_type: datatypes.Utf8Like | None = None,
    ) -> None:
        """
        Create a new instance of the AssetVideo archetype.

        Parameters
        ----------
        path:
            A path to an file stored on the local filesystem. Mutually
            exclusive with `contents`.

        contents:
            The contents of the file. Can be a BufferedReader, BytesIO, or
            bytes. Mutually exclusive with `path`.

        media_type:
            The Media Type of the asset.

            For instance:
             * `video/mp4`

            If omitted, it will be guessed from the `path` (if any),
            or the viewer will try to guess from the contents (magic header).
            If the media type cannot be guessed, the viewer won't be able to render the asset.

        """

        from ..components import MediaType

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if (path is None) == (contents is None):
                raise ValueError("Must provide exactly one of 'path' or 'contents'")

            if path is None:
                blob = contents
            else:
                blob = pathlib.Path(path).read_bytes()
                if media_type is None:
                    media_type = MediaType.guess_from_path(path)

            self.__attrs_init__(blob=blob, media_type=media_type)
            return

        self.__attrs_clear__()

    def read_frame_timestamps_nanos(self: Any) -> npt.NDArray[np.int64]:
        """
        Determines the presentation timestamps of all frames inside the video.

        Throws a runtime exception if the video cannot be read.
        """
        if self.blob is not None:
            video_buffer = self.blob.as_arrow_array()
        else:
            raise RuntimeError("Asset video has no video buffer")

        if self.media_type is not None:
            media_type = self.media_type.as_arrow_array()[0].as_py()

        return np.array(bindings.asset_video_read_frame_timestamps_nanos(video_buffer, media_type), dtype=np.int64)

    @deprecated("Renamed to `read_frame_timestamps_nanos`")
    def read_frame_timestamps_ns(self: Any) -> npt.NDArray[np.int64]:
        """DEPRECATED: renamed to read_frame_timestamps_nanos."""
        return self.read_frame_timestamps_nanos()  # type: ignore[no-any-return]
