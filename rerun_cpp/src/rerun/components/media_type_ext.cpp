#include "media_type.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct MediaTypeExt {
#define MediaType MediaTypeExt

            // Don't provide a string_view constructor, std::string constructor exists and covers this.

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct media type from a null-terminated UTF8 string.
            MediaType(const char* media_type) : value(media_type) {}

            // TODO(#2388): come up with some DSL in our flatbuffers definitions so that we can
            // declare these constants directly in there.

            /// `text/plain`
            static MediaType plain_text() {
                return "text/plain";
            }

            /// `text/markdown`
            ///
            /// <https://www.iana.org/assignments/media-types/text/markdown>
            static MediaType markdown() {
                return "text/markdown";
            }

            /// [`glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf+json`.
            ///
            /// <https://www.iana.org/assignments/media-types/model/gltf+json>
            static MediaType gltf() {
                return "model/gltf+json";
            }

            /// [Binary `glTF`](https://en.wikipedia.org/wiki/GlTF): `model/gltf-binary`.
            ///
            /// <https://www.iana.org/assignments/media-types/model/gltf-binary>
            static MediaType glb() {
                return "model/gltf-binary";
            }

            /// [Wavefront `obj`](https://en.wikipedia.org/wiki/Wavefront_.obj_file): `model/obj`.
            ///
            /// <https://www.iana.org/assignments/media-types/model/obj>
            static MediaType obj() {
                return "model/obj";
            }

            /// [Stereolithography Model `stl`](https://en.wikipedia.org/wiki/STL_(file_format)): `model/stl`.
            ///
            /// Either binary or ASCII.
            /// <https://www.iana.org/assignments/media-types/model/stl>
            static MediaType stl() {
                return "model/stl";
            }

            // </CODEGEN_COPY_TO_HEADER>
        }
    };
#endif
} // namespace components
} // namespace rerun
