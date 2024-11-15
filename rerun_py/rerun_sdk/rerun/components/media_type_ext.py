from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from . import MediaType


# TODO(#2388): constants in fbs
class MediaTypeExt:
    """Extension for [MediaType][rerun.components.MediaType]."""

    TEXT: MediaType = None  # type: ignore[assignment]
    """Plain text: `text/plain`."""

    MARKDOWN: MediaType = None  # type: ignore[assignment]
    """
    Markdown: `text/markdown`.

    <https://www.iana.org/assignments/media-types/text/markdown>
    """

    # --------------------------
    # Images:

    JPEG: MediaType = None  # type: ignore[assignment]
    """
    [JPEG image](https://en.wikipedia.org/wiki/JPEG): `image/jpeg`.
    """

    PNG: MediaType = None  # type: ignore[assignment]
    """
    [PNG image](https://en.wikipedia.org/wiki/PNG): `image/png`.

    <https://www.iana.org/assignments/media-types/image/png>
    """

    # --------------------------
    # Meshes:

    GLB: MediaType = None  # type: ignore[assignment]
    """
    Binary [`glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf-binary`.

    <https://www.iana.org/assignments/media-types/model/gltf-binary>
    """

    GLTF: MediaType = None  # type: ignore[assignment]
    """
    [`glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf+json`.

    <https://www.iana.org/assignments/media-types/model/gltf+json>
    """

    OBJ: MediaType = None  # type: ignore[assignment]
    """
    [Wavefront .obj](https://en.wikipedia.org/wiki/Wavefront_.obj_file): `model/obj`.

    <https://www.iana.org/assignments/media-types/model/obj>
    """

    STL: MediaType = None  # type: ignore[assignment]
    """
    [Stereolithography Model `stl`](https://en.wikipedia.org/wiki/STL_(file_format)): `model/stl`.
    Either binary or ASCII.

    <https://www.iana.org/assignments/media-types/model/stl>
    """

    # --------------------------
    # Video:

    MP4: MediaType = None  # type: ignore[assignment]
    """
    [`mp4`](https://en.wikipedia.org/wiki/MP4_file_format): `video/mp4`.

    <https://www.iana.org/assignments/media-types/video/mp4>
    """

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.TEXT = cls("text/plain")
        cls.MARKDOWN = cls("text/markdown")

        cls.JPEG = cls("image/jpeg")
        cls.PNG = cls("image/png")

        cls.GLB = cls("model/gltf-binary")
        cls.GLTF = cls("model/gltf+json")
        cls.OBJ = cls("model/obj")
        cls.STL = cls("model/stl")

        cls.MP4 = cls("video/mp4")

    @staticmethod
    def guess_from_path(path: str | Path) -> MediaType | None:
        from ..components import MediaType

        ext = Path(path).suffix.lower()

        # Images
        if ext == ".jpg" or ext == ".jpeg":
            return MediaType.JPEG
        elif ext == ".png":
            return MediaType.PNG

        # 3D Models
        if ext == ".glb":
            return MediaType.GLB
        elif ext == ".gltf":
            return MediaType.GLTF
        elif ext == ".obj":
            return MediaType.OBJ
        elif ext == ".stl":
            return MediaType.STL

        # Video
        if ext == ".mp4":
            return MediaType.MP4

        return None
