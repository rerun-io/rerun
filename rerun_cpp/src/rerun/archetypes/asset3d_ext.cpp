#include <algorithm>
#include <fstream>
#include <string>

#include "asset3d.hpp"

// It's undefined behavior to pre-declare std types, see http://www.gotw.ca/gotw/034.htm
// We want to use `std::filesystem::path`, so we have it include it in the header.
// <CODEGEN_COPY_TO_HEADER>

#include <filesystem>

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::archetypes {

#if 0
        // <CODEGEN_COPY_TO_HEADER>

        static std::optional<rerun::components::MediaType> guess_media_type(
            const std::filesystem::path& path
        );

        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Result<Asset3D> from_file(const std::filesystem::path& path);

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_bytes(
            rerun::Collection<uint8_t> bytes, std::optional<rerun::components::MediaType> media_type
        ) {
            // TODO(cmc): we could try and guess using magic bytes here, like rust does.
            Asset3D asset = Asset3D(std::move(bytes));
            asset.media_type = media_type;
            return asset;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

    Result<Asset3D> Asset3D::from_file(const std::filesystem::path& path) {
        std::ifstream file(path, std::ios::binary);
        if (!file) {
            return Error(ErrorCode::FileOpenFailure, "Failed to open file: " + path.string());
        }

        file.seekg(0, std::ios::end);
        std::streampos length = file.tellg();
        file.seekg(0, std::ios::beg);

        std::vector<uint8_t> data(static_cast<size_t>(length));
        file.read(reinterpret_cast<char*>(data.data()), length);

        return Asset3D::from_bytes(
            Collection<uint8_t>::take_ownership(std::move(data)),
            Asset3D::guess_media_type(path)
        );
    }

    std::optional<rerun::components::MediaType> Asset3D::guess_media_type(
        const std::filesystem::path& path
    ) {
        std::filesystem::path file_path(path);
        std::string ext = file_path.extension().string();
        std::transform(ext.begin(), ext.end(), ext.begin(), ::tolower);

        if (ext == ".glb") {
            return rerun::components::MediaType::glb();
        } else if (ext == ".gltf") {
            return rerun::components::MediaType::gltf();
        } else if (ext == ".obj") {
            return rerun::components::MediaType::obj();
        } else if (ext == ".stl") {
            return rerun::components::MediaType::stl();
        } else {
            return std::nullopt;
        }
    }
} // namespace rerun::archetypes
