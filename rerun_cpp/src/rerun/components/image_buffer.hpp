// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/image_buffer.fbs".

#pragma once

#include "../collection.hpp"
#include "../datatypes/blob.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace rerun::components {
    /// **Component**: A buffer that is known to store image data.
    ///
    /// To interpret the contents of this buffer, see, `components::ImageFormat`.
    struct ImageBuffer {
        rerun::datatypes::Blob buffer;

      public: // START of extensions from image_buffer_ext.cpp:
        /// Number of bytes
        size_t size() const {
            return buffer.size();
        }

        // END of extensions from image_buffer_ext.cpp, start of generated code:

      public:
        ImageBuffer() = default;

        ImageBuffer(rerun::datatypes::Blob buffer_) : buffer(std::move(buffer_)) {}

        ImageBuffer& operator=(rerun::datatypes::Blob buffer_) {
            buffer = std::move(buffer_);
            return *this;
        }

        ImageBuffer(rerun::Collection<uint8_t> data_) : buffer(std::move(data_)) {}

        ImageBuffer& operator=(rerun::Collection<uint8_t> data_) {
            buffer = std::move(data_);
            return *this;
        }

        /// Cast to the underlying Blob datatype
        operator rerun::datatypes::Blob() const {
            return buffer;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Blob) == sizeof(components::ImageBuffer));

    /// \private
    template <>
    struct Loggable<components::ImageBuffer> {
        static constexpr const char Name[] = "rerun.components.ImageBuffer";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Blob>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::ImageBuffer` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::ImageBuffer* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Blob>::to_arrow(&instances->buffer, num_instances);
        }
    };
} // namespace rerun
