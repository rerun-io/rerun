#include <utility>
#include "image_format.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../image_utils.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ImageFormatExt {
            uint32_t width;
            uint32_t height;
            std::optional<rerun::datatypes::PixelFormat> pixel_format;
            std::optional<rerun::datatypes::ColorModel> color_model;
            std::optional<rerun::datatypes::ChannelDatatype> channel_datatype;

#define KeypointPair ImageFormatExt

            // <CODEGEN_COPY_TO_HEADER>

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
                if (pixel_format) {
                    return pixel_format_num_bytes({width, height}, *pixel_format);
                } else {
                    auto cm = color_model.value_or(datatypes::ColorModel::L);
                    auto dt = channel_datatype.value_or(datatypes::ChannelDatatype::U8);
                    auto bits_per_pixel = color_model_channel_count(cm) * datatype_bits(dt);
                    return (width * height * bits_per_pixel + 7) / 8; // Rounding up
                }
            }

            // </CODEGEN_COPY_TO_HEADER>
        };

#endif
    } // namespace datatypes
} // namespace rerun
