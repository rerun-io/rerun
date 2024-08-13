// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/image_format.fbs".

#pragma once

#include "../datatypes/image_format.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: The metadata describing the contents of a `components::ImageBuffer`.
    struct ImageFormat {
        rerun::datatypes::ImageFormat image_format;

      public: // START of extensions from image_format_ext.cpp:
        /// From a specific pixel format.
        ImageFormat(rerun::WidthHeight resolution, datatypes::PixelFormat pixel_format)
            : image_format(resolution, pixel_format) {}

        /// Create a new image format for depth or segmentation images with the given resolution and datatype.
        ImageFormat(rerun::WidthHeight resolution, datatypes::ChannelDatatype datatype)
            : image_format(resolution, datatype) {}

        ImageFormat(
            rerun::WidthHeight resolution, datatypes::ColorModel color_model,
            datatypes::ChannelDatatype datatype
        )
            : image_format(resolution, color_model, datatype) {}

        // END of extensions from image_format_ext.cpp, start of generated code:

      public:
        ImageFormat() = default;

        ImageFormat(rerun::datatypes::ImageFormat image_format_) : image_format(image_format_) {}

        ImageFormat& operator=(rerun::datatypes::ImageFormat image_format_) {
            image_format = image_format_;
            return *this;
        }

        /// Cast to the underlying ImageFormat datatype
        operator rerun::datatypes::ImageFormat() const {
            return image_format;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::ImageFormat) == sizeof(components::ImageFormat));

    /// \private
    template <>
    struct Loggable<components::ImageFormat> {
        static constexpr const char Name[] = "rerun.components.ImageFormat";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::ImageFormat>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::ImageFormat` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::ImageFormat* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::ImageFormat>::to_arrow(
                &instances->image_format,
                num_instances
            );
        }
    };
} // namespace rerun
