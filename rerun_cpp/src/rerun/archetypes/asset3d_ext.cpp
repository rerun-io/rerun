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

        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        /// \deprecated Use `from_file_path` instead.
        [[deprecated("Use `from_file_path` instead")]] static Result<Asset3D> from_file(
            const std::filesystem::path& path
        );

        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Result<Asset3D> from_file_path(const std::filesystem::path& path);

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        /// \deprecated Use `from_file_contents` instead.
        [[deprecated("Use `from_file_contents` instead")]] static Asset3D from_bytes(
            rerun::Collection<uint8_t> bytes,
            std::optional<rerun::components::MediaType> media_type = {}
        ) {
            return from_file_contents(bytes, media_type);
        }

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_file_contents(
            rerun::Collection<uint8_t> bytes,
            std::optional<rerun::components::MediaType> media_type = {}
        ) {
            Asset3D asset = Asset3D(std::move(bytes));
            // TODO(cmc): we could try and guess using magic bytes here, like rust does.
            if (media_type.has_value()) {
                return std::move(asset).with_media_type(media_type.value());
            }
            return asset;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

    Result<Asset3D> Asset3D::from_file(const std::filesystem::path& path) {
        return from_file_path(path);
    }

    Result<Asset3D> Asset3D::from_file_path(const std::filesystem::path& path) {
        std::ifstream file(path, std::ios::binary);
        if (!file) {
            return Error(ErrorCode::FileOpenFailure, "Failed to open file: " + path.string());
        }

        file.seekg(0, std::ios::end);
        std::streampos length = file.tellg();
        file.seekg(0, std::ios::beg);

        std::vector<uint8_t> data(static_cast<size_t>(length));
        file.read(reinterpret_cast<char*>(data.data()), length);

        return Asset3D::from_file_contents(
            Collection<uint8_t>::take_ownership(std::move(data)),
            rerun::components::MediaType::guess_from_path(path)
        );
    }
} // namespace rerun::archetypes
