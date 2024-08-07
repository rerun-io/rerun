// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/image.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/blob.hpp"
#include "../components/draw_order.hpp"
#include "../components/image_format.hpp"
#include "../components/opacity.hpp"
#include "../data_cell.hpp"
#include "../image_utils.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A monochrome or color image.
    ///
    /// See also `archetypes::DepthImage` and `archetypes::SegmentationImage`.
    ///
    /// The raw image data is stored as a single buffer of bytes in a `components::Blob`.
    /// The meaning of these bytes is determined by the `components::ImageFormat` which specifies the resolution
    /// and the pixel format (e.g. RGB, RGBA, …).
    ///
    /// The order of dimensions in the underlying `components::Blob` follows the typical
    /// row-major, interleaved-pixel image format.
    ///
    /// Rerun also supports compressed images (JPEG, PNG, …), using `archetypes::EncodedImage`.
    /// Compressing images can save a lot of bandwidth and memory.
    ///
    /// Since the underlying [rerun::components::Blob] uses `rerun::Collection` internally,
    /// data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
    /// If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
    ///
    /// ## Example
    ///
    /// ### image_simple:
    /// ![image](https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_image");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Create a synthetic image.
    ///     const int HEIGHT = 200;
    ///     const int WIDTH = 300;
    ///     std::vector<uint8_t> data(WIDTH * HEIGHT * 3, 0);
    ///     for (size_t i = 0; i <data.size(); i += 3) {
    ///         data[i] = 255;
    ///     }
    ///     for (size_t y = 50; y <150; ++y) {
    ///         for (size_t x = 50; x <150; ++x) {
    ///             data[(y * WIDTH + x) * 3 + 0] = 0;
    ///             data[(y * WIDTH + x) * 3 + 1] = 255;
    ///             data[(y * WIDTH + x) * 3 + 2] = 0;
    ///         }
    ///     }
    ///
    ///     rec.log("image", rerun::Image::from_rgb24(data, {WIDTH, HEIGHT}));
    /// }
    /// ```
    struct Image {
        /// The raw image data.
        rerun::components::Blob data;

        /// The format of the image.
        rerun::components::ImageFormat format;

        /// Opacity of the image, useful for layering several images.
        ///
        /// Defaults to 1.0 (fully opaque).
        std::optional<rerun::components::Opacity> opacity;

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<rerun::components::DrawOrder> draw_order;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.ImageIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public: // START of extensions from image_ext.cpp:
        /// Construct an image from bytes and image format.
        ///
        /// @param bytes The raw image data as bytes.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * image_format.bytes_per_pixel`.
        /// @param format_ How the data should be interpreted.
        Image(Collection<uint8_t> bytes, components::ImageFormat format_)
            : data(std::move(bytes)), format(format_) {}

        /// Construct an image from resolution, pixel format and bytes.
        ///
        /// @param bytes The raw image data as bytes.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * pixel_format.bytes_per_pixel`.
        /// @param resolution The resolution of the image as {width, height}.
        /// @param pixel_format How the data should be interpreted.
        Image(
            Collection<uint8_t> bytes, WidthHeight resolution, datatypes::PixelFormat pixel_format
        )
            : Image{std::move(bytes), datatypes::ImageFormat{resolution, pixel_format}} {}

        /// Construct an image from resolution, color model, channel datatype and bytes.
        ///
        /// @param bytes The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * datatype.bytes * color_model.num_channels`.
        /// @param resolution The resolution of the image as {width, height}.
        /// @param color_model The color model of the pixel data.
        /// @param datatype Datatype of the individual channels of the color model.
        Image(
            Collection<uint8_t> bytes, WidthHeight resolution, datatypes::ColorModel color_model,
            datatypes::ChannelDatatype datatype
        )
            : Image(std::move(bytes), datatypes::ImageFormat(resolution, color_model, datatype)) {}

        /// Construct an image from resolution, color model and elements,
        /// inferring the channel datatype from the element type.
        ///
        /// @param elements Pixel data as a `rerun::Collection`.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * color_model.num_channels`.
        /// @param resolution The resolution of the image as {width, height}.
        /// @param color_model The color model of the pixel data.
        /// Each element in elements is interpreted as a single channel of the color model.
        template <typename T>
        Image(Collection<T> elements, WidthHeight resolution, datatypes::ColorModel color_model)
            : Image(elements.to_uint8(), resolution, color_model, get_datatype(elements.data())) {}

        /// Construct an image from resolution, color model and element pointer,
        /// inferring the channel datatype from the element type.
        ///
        /// @param elements The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The number of elements is assumed to be `W * H * color_model.num_channels`.
        /// @param resolution The resolution of the image as {width, height}.
        /// @param color_model The color model of the pixel data.
        /// Each element in elements is interpreted as a single channel of the color model.
        template <typename T>
        Image(const T* elements, WidthHeight resolution, datatypes::ColorModel color_model)
            : Image(
                  rerun::Collection<uint8_t>::borrow(
                      reinterpret_cast<const uint8_t*>(elements),
                      resolution.width * resolution.height * color_model_channel_count(color_model)
                  ),
                  resolution, color_model, get_datatype(elements)
              ) {}

        /// Assumes single channel greyscale/luminance with 8-bit per value.
        ///
        /// @param bytes Pixel data as a `rerun::Collection`.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        /// @param resolution The resolution of the image as {width, height}.
        static Image from_greyscale8(Collection<uint8_t> bytes, WidthHeight resolution) {
            return Image(
                bytes,
                resolution,
                datatypes::ColorModel::L,
                datatypes::ChannelDatatype::U8
            );
        }

        /// Assumes RGB, 8-bit per channel, packed as `RGBRGBRGB…`.
        ///
        /// @param bytes Pixel data as a `rerun::Collection`.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * 3`.
        /// @param resolution The resolution of the image as {width, height}.
        static Image from_rgb24(Collection<uint8_t> bytes, WidthHeight resolution) {
            return Image(
                bytes,
                resolution,
                datatypes::ColorModel::RGB,
                datatypes::ChannelDatatype::U8
            );
        }

        /// Assumes RGBA, 8-bit per channel, with separate alpha.
        ///
        /// @param bytes Pixel data as a `rerun::Collection`.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H * 4`.
        /// @param resolution The resolution of the image as {width, height}.
        static Image from_rgba32(Collection<uint8_t> bytes, WidthHeight resolution) {
            return Image(
                bytes,
                resolution,
                datatypes::ColorModel::RGBA,
                datatypes::ChannelDatatype::U8
            );
        }

        // END of extensions from image_ext.cpp, start of generated code:

      public:
        Image() = default;
        Image(Image&& other) = default;

        /// Opacity of the image, useful for layering several images.
        ///
        /// Defaults to 1.0 (fully opaque).
        Image with_opacity(rerun::components::Opacity _opacity) && {
            opacity = std::move(_opacity);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        Image with_draw_order(rerun::components::DrawOrder _draw_order) && {
            draw_order = std::move(_draw_order);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Image> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::Image& archetype);
    };
} // namespace rerun
