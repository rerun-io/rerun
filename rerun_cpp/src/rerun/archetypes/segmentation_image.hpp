// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/segmentation_image.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/blob.hpp"
#include "../components/channel_datatype.hpp"
#include "../components/draw_order.hpp"
#include "../components/opacity.hpp"
#include "../components/resolution2d.hpp"
#include "../data_cell.hpp"
#include "../image_utils.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: An image made up of integer `components::ClassId`s.
    ///
    /// Each pixel corresponds to a `components::ClassId` that will be mapped to a color based on annotation context.
    ///
    /// In the case of floating point images, the label will be looked up based on rounding to the nearest
    /// integer value.
    ///
    /// See also `archetypes::AnnotationContext` to associate each class with a color and a label.
    ///
    /// Since the underlying `rerun::datatypes::TensorData` uses `rerun::Collection` internally,
    /// data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
    /// If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
    ///
    /// ## Example
    ///
    /// ### Simple segmentation image
    /// ![image](https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm> // std::fill_n
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_segmentation_image");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Create a segmentation image
    ///     const int HEIGHT = 8;
    ///     const int WIDTH = 12;
    ///     std::vector<uint8_t> data(WIDTH * HEIGHT, 0);
    ///     for (auto y = 0; y <4; ++y) {                                         // top half
    ///         std::fill_n(data.begin() + y * WIDTH, 6, static_cast<uint8_t>(1)); // left half
    ///     }
    ///     for (auto y = 4; y <8; ++y) {                                             // bottom half
    ///         std::fill_n(data.begin() + y * WIDTH + 6, 6, static_cast<uint8_t>(2)); // right half
    ///     }
    ///
    ///     // create an annotation context to describe the classes
    ///     rec.log_static(
    ///         "/",
    ///         rerun::AnnotationContext({
    ///             rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
    ///             rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
    ///         })
    ///     );
    ///
    ///     rec.log("image", rerun::SegmentationImage(data, {WIDTH, HEIGHT}));
    /// }
    /// ```
    struct SegmentationImage {
        /// The raw image data.
        rerun::components::Blob data;

        /// The size of the image.
        rerun::components::Resolution2D resolution;

        /// The data type of the segmentation image data (U16, U32, …).
        rerun::components::ChannelDatatype datatype;

        /// Opacity of the image, useful for layering the segmentation image on top of another image.
        ///
        /// Defaults to 0.5 if there's any other images in the scene, otherwise 1.0.
        std::optional<rerun::components::Opacity> opacity;

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<rerun::components::DrawOrder> draw_order;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SegmentationImageIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public: // START of extensions from segmentation_image_ext.cpp:
        /// Constructs image from pointer + resolution, inferring the datatype from the pointer type.
        ///
        /// @param pixels The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The length of the data should be `W * H`.
        template <typename TElement>
        SegmentationImage(const TElement* pixels, components::Resolution2D resolution_)
            : SegmentationImage{
                  reinterpret_cast<const uint8_t*>(pixels), resolution_, get_datatype(pixels)} {}

        /// Constructs image from pixel data + resolution with datatype inferred from the passed collection.
        ///
        /// @param pixels The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        template <typename TElement>
        SegmentationImage(Collection<TElement> pixels, components::Resolution2D resolution_)
            : SegmentationImage{pixels.to_uint8(), resolution_, get_datatype(pixels.data())} {}

        /// Constructs image from pixel data + resolution with explicit datatype. Borrows data from a pointer (i.e. data must outlive the image!).
        ///
        /// @param data_ The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The length of the data should be `W * H * datatype.size`
        SegmentationImage(
            const void* data_, components::Resolution2D resolution_,
            components::ChannelDatatype datatype_
        )
            : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, datatype_))},
              resolution{resolution_},
              datatype{datatype_} {}

        /// Constructs image from pixel data + resolution + datatype.
        ///
        /// @param pixels The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        SegmentationImage(
            Collection<uint8_t> data_, components::Resolution2D resolution_,
            components::ChannelDatatype datatype_
        )
            : data{data_}, resolution{resolution_}, datatype{datatype_} {}

        // END of extensions from segmentation_image_ext.cpp, start of generated code:

      public:
        SegmentationImage() = default;
        SegmentationImage(SegmentationImage&& other) = default;

        /// Opacity of the image, useful for layering the segmentation image on top of another image.
        ///
        /// Defaults to 0.5 if there's any other images in the scene, otherwise 1.0.
        SegmentationImage with_opacity(rerun::components::Opacity _opacity) && {
            opacity = std::move(_opacity);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        SegmentationImage with_draw_order(rerun::components::DrawOrder _draw_order) && {
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
    struct AsComponents<archetypes::SegmentationImage> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const archetypes::SegmentationImage& archetype
        );
    };
} // namespace rerun
