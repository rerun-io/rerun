#include <algorithm>
#include <fstream>
#include <string>

#include "../c/rerun.h"
#include "../string_utils.hpp"
#include "asset_video.hpp"

#include <arrow/array/array_binary.h>
#include <arrow/array/array_nested.h>

// It's undefined behavior to pre-declare std types, see http://www.gotw.ca/gotw/034.htm
// We want to use `std::filesystem::path`, so we have it include it in the header.
// <CODEGEN_COPY_TO_HEADER>

#include <chrono>
#include <filesystem>

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::archetypes {

#if 0
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new `AssetVideo` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Result<AssetVideo> from_file(const std::filesystem::path& path);

        /// Creates a new `AssetVideo` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static AssetVideo from_bytes(
            rerun::Collection<uint8_t> bytes, std::optional<rerun::components::MediaType> media_type = {}
        ) {
            AssetVideo asset = AssetVideo(std::move(bytes));
            // TODO(jan): we could try and guess using magic bytes here, like rust does.
            if (media_type.has_value()) {
                return std::move(asset).with_media_type(media_type.value());
            }
            return asset;
        }

        /// Determines the presentation timestamps of all frames inside the video.
        ///
        /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
        Result<std::vector<std::chrono::nanoseconds>> read_frame_timestamps_nanos() const;

        /// DEPRECATED: Use `read_frame_timestamps_nanos` instead.
        [[deprecated("Renamed to `read_frame_timestamps_nanos`"
        )]] Result<std::vector<std::chrono::nanoseconds>>
            read_frame_timestamps_ns() const {
            return read_frame_timestamps_nanos();
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

    Result<AssetVideo> AssetVideo::from_file(const std::filesystem::path& path) {
        std::ifstream file(path, std::ios::binary);
        if (!file) {
            return Error(ErrorCode::FileOpenFailure, "Failed to open file: " + path.string());
        }

        file.seekg(0, std::ios::end);
        std::streampos length = file.tellg();
        file.seekg(0, std::ios::beg);

        std::vector<uint8_t> data(static_cast<size_t>(length));
        file.read(reinterpret_cast<char*>(data.data()), length);

        return AssetVideo::from_bytes(
            Collection<uint8_t>::take_ownership(std::move(data)),
            rerun::components::MediaType::guess_from_path(path)
        );
    }

    static int64_t* alloc_timestamps(void* alloc_context, uint32_t num_timestamps) {
        auto frame_timestamps_ptr =
            static_cast<std::vector<std::chrono::nanoseconds>*>(alloc_context);
        frame_timestamps_ptr->resize(num_timestamps);
        return reinterpret_cast<int64_t*>(frame_timestamps_ptr->data());
    }

    Result<std::vector<std::chrono::nanoseconds>> AssetVideo::read_frame_timestamps_nanos() const {
        static_assert(sizeof(int64_t) == sizeof(std::chrono::nanoseconds::rep));

        if (!blob.has_value()) {
            return std::vector<std::chrono::nanoseconds>();
        }
        auto blob_list_array = std::dynamic_pointer_cast<arrow::ListArray>(blob.value().array);
        if (!blob_list_array) {
            return Error(ErrorCode::InvalidComponent, "Blob array is not a primitive array");
        }
        auto blob_array =
            std::dynamic_pointer_cast<arrow::PrimitiveArray>(blob_list_array->values());
        if (!blob_array) {
            return Error(ErrorCode::InvalidComponent, "Blob array is not a primitive array");
        }
        auto blob_array_data = blob_array->values();

        rr_string media_type_c = detail::to_rr_string(std::nullopt);
        if (media_type.has_value()) {
            auto media_type_array =
                std::dynamic_pointer_cast<arrow::StringArray>(media_type.value().array);
            if (media_type_array) {
                media_type_c = detail::to_rr_string(media_type_array->GetView(0));
            }
        }

        std::vector<std::chrono::nanoseconds> frame_timestamps;

        rr_error status = {};
        rr_video_asset_read_frame_timestamps_nanos(
            blob_array_data->data(),
            static_cast<uint64_t>(blob_array_data->size()),
            media_type_c,
            &frame_timestamps,
            &alloc_timestamps,
            &status
        );
        if (status.code != RR_ERROR_CODE_OK) {
            return Error(status);
        }

        return frame_timestamps;
    }

} // namespace rerun::archetypes
