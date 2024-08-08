#pragma once

#include <arrow/buffer.h>

// Do not include this file in any public header as we don't want to infect the user's namespace
// with symbols from arrow.
// (in order to keep compile times low and avoid potential arrow version conflicts)

namespace rerun {
    /// Creates an arrow buffer from a vector without allocating new memory.
    ///
    /// Newer version of the arrow sdk have this builtin as `arrow::Buffer::FromVector`.
    template <typename T>
    inline std::shared_ptr<arrow::Buffer> arrow_buffer_from_vector(std::vector<T> vec) {
        static_assert(
            std::is_trivial_v<T>,
            "Buffer::FromVector can only wrap vectors of trivial objects"
        );

        if (vec.empty()) {
            return std::make_shared<arrow::Buffer>(nullptr, 0);
        }

        auto* data = reinterpret_cast<uint8_t*>(vec.data());
        auto size_in_bytes = static_cast<int64_t>(vec.size() * sizeof(T));
        return std::shared_ptr<arrow::Buffer>{
            new arrow::Buffer{data, size_in_bytes},
            // Keep the vector's buffer alive inside the shared_ptr's destructor until after we have deleted the Buffer.
            [vec = std::move(vec)](arrow::Buffer* buffer) { delete buffer; }};
    }

} // namespace rerun
