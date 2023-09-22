from __future__ import annotations

from typing import TYPE_CHECKING, Final

if TYPE_CHECKING:
    from . import MediaType


# TODO(#2388): constants in fbs
class MediaTypeExt:
    TEXT: Final = "text/plain"
    MARKDOWN: Final = "text/markdown"
    GLB: Final = "model/gltf-binary"
    GLTF: Final = "model/gltf+json"
    OBJ: Final = "model/obj"

    # TODO(cmc): these should just be constants, but circular import hell...

    @staticmethod
    def plain_text() -> MediaType:
        """Plain text: `text/plain`."""
        from . import MediaType

        return MediaType(MediaTypeExt.TEXT)

    @staticmethod
    def markdown() -> MediaType:
        """
        Markdown: `text/markdown`.

        <https://www.iana.org/assignments/media-types/text/markdown>
        """
        from . import MediaType

        return MediaType(MediaTypeExt.MARKDOWN)

    @staticmethod
    def glb() -> MediaType:
        """
        Binary [`glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf-binary`.

        <https://www.iana.org/assignments/media-types/model/gltf-binary>
        """
        from . import MediaType

        return MediaType(MediaTypeExt.GLB)

    @staticmethod
    def gltf() -> MediaType:
        """
        [`glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf+json`.

        <https://www.iana.org/assignments/media-types/model/gltf+json>
        """
        from . import MediaType

        return MediaType(MediaTypeExt.GLTF)

    @staticmethod
    def obj() -> MediaType:
        """
        [Wavefront .obj](https://en.wikipedia.org/wiki/Wavefront_.obj_file): `model/obj`.

        <https://www.iana.org/assignments/media-types/model/obj>
        """
        from . import MediaType

        return MediaType(MediaTypeExt.OBJ)
