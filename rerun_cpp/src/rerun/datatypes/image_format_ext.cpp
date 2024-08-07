#include <utility>
#include "image_format.hpp"
#include "image_utils.hpp"

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

            /// From a speicifc pixel format.
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

            // </CODEGEN_COPY_TO_HEADER>
        };

#endif
    } // namespace datatypes
} // namespace rerun
