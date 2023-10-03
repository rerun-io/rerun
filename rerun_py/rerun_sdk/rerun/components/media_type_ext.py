from __future__ import annotations

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

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.TEXT = cls("text/plain")
        cls.MARKDOWN = cls("text/markdown")
        cls.GLB = cls("model/gltf-binary")
        cls.GLTF = cls("model/gltf+json")
        cls.OBJ = cls("model/obj")
