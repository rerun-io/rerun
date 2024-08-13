// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/image_format.fbs".

#pragma once

#include "../image_utils.hpp"
#include "../result.hpp"
#include "channel_datatype.hpp"
#include "color_model.hpp"
#include "pixel_format.hpp"

#include <cstdint>
#include <memory>
#include <optional>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: The metadata describing the contents of a `components::ImageBuffer`.
    struct ImageFormat {
        /// The width of the image in pixels.
        uint32_t width;

        /// The height of the image in pixels.
        uint32_t height;

        /// Used mainly for chroma downsampled formats and differing number of bits per channel.
        ///
        /// If specified, this takes precedence over both `datatypes::ColorModel` and `datatypes::ChannelDatatype` (which are ignored).
        std::optional<rerun::datatypes::PixelFormat> pixel_format;

        /// L, RGB, RGBA, …
        ///
        /// Also requires a `datatypes::ChannelDatatype` to fully specify the pixel format.
        std::optional<rerun::datatypes::ColorModel> color_model;

        /// The data type of each channel (e.g. the red channel) of the image data (U8, F16, …).
        ///
        /// Also requires a `datatypes::ColorModel` to fully specify the pixel format.
        std::optional<rerun::datatypes::ChannelDatatype> channel_datatype;

      public: // START of extensions from image_format_ext.cpp:
        /// From a specific pixel format.
        ImageFormat(rerun::WidthHeight resolution, datatypes::PixelFormat pixel_format_)
            : width(resolution.width), height(resolution.height), pixel_format(pixel_format_) {}

        /// Create a new image format for depth or segmentation images with the given resolution and datatype.
        ImageFormat(rerun::WidthHeight resolution, datatypes::ChannelDatatype datatype_)
            : width(resolution.width), height(resolution.height), channel_datatype(datatype_) {}

        ImageFormat(
            rerun::WidthHeight resolution, datatypes::ColorModel color_model_,
            datatypes::ChannelDatatype datatype_
        )
            : width(resolution.width),
              height(resolution.height),
              color_model(color_model_),
              channel_datatype(datatype_) {}

        /// How many bytes will this image occupy?
        size_t num_bytes() const {
            return width * height * this->bits_per_pixel() / 8;
        }

        /// How many bits per pixel?
        ///
        /// Note that this is not necessarily a factor of 8.
        size_t bits_per_pixel() const {
            if (pixel_format) {
                return pixel_format_bits_per_pixel(*pixel_format);
            } else {
                auto cm = color_model.value_or(datatypes::ColorModel());
                auto dt = channel_datatype.value_or(datatypes::ChannelDatatype());
                return color_model_channel_count(cm) * datatype_bits(dt);
            }
        }

        // END of extensions from image_format_ext.cpp, start of generated code:

      public:
        ImageFormat() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::ImageFormat> {
        static constexpr const char Name[] = "rerun.datatypes.ImageFormat";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::ImageFormat` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::ImageFormat* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::ImageFormat* elements,
            size_t num_elements
        );
    };
} // namespace rerun
