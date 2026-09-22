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

    PLY: MediaType = None  # type: ignore[assignment]
    """
    [PLY (Polygon File Format)](https://en.wikipedia.org/wiki/PLY_(file_format)): `application/x-ply`.

    Holds either a mesh or a point cloud, depending on its header.
    """

    STL: MediaType = None  # type: ignore[assignment]
    """
    [Stereolithography Model `stl`](https://en.wikipedia.org/wiki/STL_(file_format)): `model/stl`.
    Either binary or ASCII.

    <https://www.iana.org/assignments/media-types/model/stl>
    """

    # --------------------------
    # Compressed Depth Data:

    RVL: MediaType = None  # type: ignore[assignment]
    """
    RVL compressed depth: `application/rvl`.

    Run length encoding and Variable Length encoding schemes (RVL) compressed depth data format.
    <https://www.microsoft.com/en-us/research/wp-content/uploads/2018/09/p100-wilson.pdf>
    """

    # --------------------------
    # Video:

    MP4: MediaType = None  # type: ignore[assignment]
    """
    [`mp4`](https://en.wikipedia.org/wiki/MP4_file_format): `video/mp4`.

    <https://www.iana.org/assignments/media-types/video/mp4>
    """

    # --------------------------
    # Audio:

    AAC: MediaType = None  # type: ignore[assignment]
    """
    [AAC audio](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) in a raw ADTS stream: `audio/aac`.

    <https://www.iana.org/assignments/media-types/audio/aac>
    """

    FLAC: MediaType = None  # type: ignore[assignment]
    """
    [FLAC audio](https://en.wikipedia.org/wiki/FLAC): `audio/flac`.
    """

    M4A: MediaType = None  # type: ignore[assignment]
    """
    [M4A audio](https://en.wikipedia.org/wiki/MP4_file_format) (AAC in an MP4 container): `audio/mp4`.

    <https://www.iana.org/assignments/media-types/audio/mp4>
    """

    MP3: MediaType = None  # type: ignore[assignment]
    """
    [MP3 audio](https://en.wikipedia.org/wiki/MP3): `audio/mpeg`.

    <https://www.iana.org/assignments/media-types/audio/mpeg>
    """

    OGG: MediaType = None  # type: ignore[assignment]
    """
    [Ogg audio](https://en.wikipedia.org/wiki/Ogg) (Vorbis or Opus): `audio/ogg`.

    <https://www.iana.org/assignments/media-types/audio/ogg>
    """

    WAV: MediaType = None  # type: ignore[assignment]
    """
    [WAV audio](https://en.wikipedia.org/wiki/WAV): `audio/wav`.
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
        cls.PLY = cls("application/x-ply")
        cls.STL = cls("model/stl")

        cls.RVL = cls("application/rvl")

        cls.MP4 = cls("video/mp4")

        cls.AAC = cls("audio/aac")
        cls.FLAC = cls("audio/flac")
        cls.M4A = cls("audio/mp4")
        cls.MP3 = cls("audio/mpeg")
        cls.OGG = cls("audio/ogg")
        cls.WAV = cls("audio/wav")

    @staticmethod
    def guess_from_path(path: str | Path) -> MediaType | None:
        from ..components import MediaType

        ext = Path(path).suffix.lower()

        # Images
        if ext in {".jpg", ".jpeg"}:
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
        elif ext == ".ply":
            return MediaType.PLY
        elif ext == ".stl":
            return MediaType.STL

        # Video
        if ext == ".mp4":
            return MediaType.MP4

        # Audio
        if ext == ".aac":
            return MediaType.AAC
        elif ext == ".flac":
            return MediaType.FLAC
        elif ext == ".m4a":
            return MediaType.M4A
        elif ext == ".mp3":
            return MediaType.MP3
        elif ext in {".oga", ".ogg", ".opus"}:
            return MediaType.OGG
        elif ext == ".wav":
            return MediaType.WAV

        return None
