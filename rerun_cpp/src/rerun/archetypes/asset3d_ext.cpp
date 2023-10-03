#include <filesystem>
#include <fstream>
#include <string>

#include "asset3d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        static std::optional<rerun::components::MediaType> guess_media_type(
            const std::string& path //
        ) {
            std::filesystem::path file_path(path);
            std::string ext = file_path.extension().string();

            if (ext == ".glb") {
                return rerun::components::MediaType::glb();
            } else if (ext == ".gltf") {
                return rerun::components::MediaType::gltf();
            } else if (ext == ".obj") {
                return rerun::components::MediaType::obj();
            } else {
                return std::nullopt;
            }
        }

        /// Creates a new [`Asset3D`] from the file contents at `path`.
        ///
        /// The [`MediaType`] will be guessed from the file extension.
        ///
        /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_file(const std::filesystem::path& path) {
            std::ifstream file(path, std::ios::binary);
            if (!file) {
                throw std::runtime_error("Failed to open file: " + path.string());
            }

            file.seekg(0, std::ios::end);
            std::streampos length = file.tellg();
            file.seekg(0, std::ios::beg);

            std::vector<uint8_t> data(static_cast<size_t>(length));
            file.read(reinterpret_cast<char*>(data.data()), length);

            return Asset3D::from_bytes(data, Asset3D::guess_media_type(path));
        }

        /// Creates a new [`Asset3D`] from the given `bytes`.
        ///
        /// If no [`MediaType`] is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_bytes(
            const std::vector<uint8_t> bytes, std::optional<rerun::components::MediaType> media_type
        ) {
            // TODO(cmc): we could try and guess using magic bytes here, like rust does.
            Asset3D asset = Asset3D(bytes);
            asset.media_type = media_type;
            return asset;
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
