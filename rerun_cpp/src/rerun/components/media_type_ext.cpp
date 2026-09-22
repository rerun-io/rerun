#include <algorithm>
#include <optional>
#include <string>
#include "media_type.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

// It's undefined behavior to pre-declare std types, see http://www.gotw.ca/gotw/034.htm
// We want to use `std::filesystem::path`, so we have it include it in the header.
// <CODEGEN_COPY_TO_HEADER>

#include <filesystem>

// </CODEGEN_COPY_TO_HEADER>

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

            // ------------------------------------------------
            // Images:

            /// [JPEG image](https://en.wikipedia.org/wiki/JPEG): `image/jpeg`.
            static MediaType jpeg() {
                return "image/jpeg";
            }

            /// [PNG image](https://en.wikipedia.org/wiki/PNG): `image/png`.
            ///
            /// <https://www.iana.org/assignments/media-types/image/png>
            static MediaType png() {
                return "image/png";
            }

            // ------------------------------------------------
            // Meshes:

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

            /// [PLY (Polygon File Format)](https://en.wikipedia.org/wiki/PLY_(file_format)): `application/x-ply`.
            ///
            /// Holds either a mesh or a point cloud, depending on its header.
            static MediaType ply() {
                return "application/x-ply";
            }

            /// [Stereolithography Model `stl`](https://en.wikipedia.org/wiki/STL_(file_format)): `model/stl`.
            ///
            /// Either binary or ASCII.
            /// <https://www.iana.org/assignments/media-types/model/stl>
            static MediaType stl() {
                return "model/stl";
            }

            // -------------------------------------------------------
            // Compressed Depth Data:

            /// RVL compressed depth: `application/rvl`.
            ///
            /// Run length encoding and Variable Length encoding schemes (RVL) compressed depth data format.
            /// <https://www.microsoft.com/en-us/research/wp-content/uploads/2018/09/p100-wilson.pdf>
            static MediaType rvl() {
                return "application/rvl";
            }

            // -------------------------------------------------------
            /// Videos:

            /// [MP4 video](https://en.wikipedia.org/wiki/MP4_file_format): `video/mp4`.
            ///
            /// <https://www.iana.org/assignments/media-types/video/mp4>
            static MediaType mp4() {
                return "video/mp4";
            }

            // -------------------------------------------------------
            // Audio:

            /// [AAC audio](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) in a raw ADTS stream: `audio/aac`.
            ///
            /// <https://www.iana.org/assignments/media-types/audio/aac>
            static MediaType aac() {
                return "audio/aac";
            }

            /// [FLAC audio](https://en.wikipedia.org/wiki/FLAC): `audio/flac`.
            static MediaType flac() {
                return "audio/flac";
            }

            /// [M4A audio](https://en.wikipedia.org/wiki/MP4_file_format) (AAC in an MP4 container): `audio/mp4`.
            ///
            /// <https://www.iana.org/assignments/media-types/audio/mp4>
            static MediaType m4a() {
                return "audio/mp4";
            }

            /// [MP3 audio](https://en.wikipedia.org/wiki/MP3): `audio/mpeg`.
            ///
            /// <https://www.iana.org/assignments/media-types/audio/mpeg>
            static MediaType mp3() {
                return "audio/mpeg";
            }

            /// [Ogg audio](https://en.wikipedia.org/wiki/Ogg) (Vorbis or Opus): `audio/ogg`.
            ///
            /// <https://www.iana.org/assignments/media-types/audio/ogg>
            static MediaType ogg() {
                return "audio/ogg";
            }

            /// [WAV audio](https://en.wikipedia.org/wiki/WAV): `audio/wav`.
            static MediaType wav() {
                return "audio/wav";
            }

            static std::optional<MediaType> guess_from_path(const std::filesystem::path& path);

            // </CODEGEN_COPY_TO_HEADER>
        }
#endif

        std::optional<MediaType>
            MediaType::guess_from_path(const std::filesystem::path& path) {
            std::filesystem::path file_path(path);
            std::string ext = file_path.extension().string();
            std::transform(ext.begin(), ext.end(), ext.begin(), ::tolower);

            // Images
            if (ext == ".jpg" || ext == ".jpeg") {
                return rerun::components::MediaType::jpeg();
            } else if (ext == ".png") {
                return rerun::components::MediaType::png();
            }

            // 3D Models
            if (ext == ".glb") {
                return MediaType::glb();
            } else if (ext == ".gltf") {
                return MediaType::gltf();
            } else if (ext == ".obj") {
                return MediaType::obj();
            } else if (ext == ".ply") {
                return MediaType::ply();
            } else if (ext == ".stl") {
                return MediaType::stl();
            }

            // Video
            if (ext == ".mp4") {
                return MediaType::mp4();
            }

            // Audio
            if (ext == ".aac") {
                return MediaType::aac();
            } else if (ext == ".flac") {
                return MediaType::flac();
            } else if (ext == ".m4a") {
                return MediaType::m4a();
            } else if (ext == ".mp3") {
                return MediaType::mp3();
            } else if (ext == ".oga" || ext == ".ogg" || ext == ".opus") {
                return MediaType::ogg();
            } else if (ext == ".wav") {
                return MediaType::wav();
            }

            return std::nullopt;
        }
    }; // namespace components
}; // namespace rerun
