// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/segmentation_image.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/draw_order.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: An image made up of integer class-ids.
    ///
    /// The shape of the `TensorData` must be mappable to an `HxW` tensor.
    /// Each pixel corresponds to a depth value in units specified by meter.
    ///
    /// Leading and trailing unit-dimensions are ignored, so that
    /// `1x640x480x1` is treated as a `640x480` image.
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
    ///     const auto rec = rerun::RecordingStream("rerun_example_annotation_context_connections");
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
    ///     rec.log_timeless(
    ///         "/",
    ///         rerun::AnnotationContext({
    ///             rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
    ///             rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
    ///         })
    ///     );
    ///
    ///     rec.log("image", rerun::SegmentationImage({HEIGHT, WIDTH}, data));
    /// }
    /// ```
    struct SegmentationImage {
        /// The image data. Should always be a rank-2 tensor.
        rerun::components::TensorData data;

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<rerun::components::DrawOrder> draw_order;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SegmentationImageIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = components::IndicatorComponent<IndicatorComponentName>;

      public:
        // Extensions to generated type defined in 'segmentation_image_ext.cpp'

        /// New segmentation image from height/width and tensor buffer.
        ///
        /// \param shape
        /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2.
        /// Sets the dimension names to "height" and "width" if they are not specified.
        /// \param buffer
        /// The tensor buffer containing the segmentation image data.
        SegmentationImage(
            Collection<datatypes::TensorDimension> shape, datatypes::TensorBuffer buffer
        )
            : SegmentationImage(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// New segmentation image from tensor data.
        ///
        /// \param data_
        /// The tensor buffer containing the segmentation image data.
        /// Sets the dimension names to "height" and "width" if they are not specified.
        /// Calls `Error::handle()` if the shape is not rank 2.
        explicit SegmentationImage(components::TensorData data_);

        /// New segmentation image from dimensions and pointer to segmentation image data.
        ///
        /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
        /// \param shape
        /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2.
        /// Sets the dimension names to "height", "width" and "channel" if they are not specified.
        /// Determines the number of elements expected to be in `data`.
        /// \param data_
        /// Target of the pointer must outlive the archetype.
        template <typename TElement>
        explicit SegmentationImage(
            Collection<datatypes::TensorDimension> shape, const TElement* data_
        )
            : SegmentationImage(datatypes::TensorData(std::move(shape), data_)) {}

      public:
        SegmentationImage() = default;
        SegmentationImage(SegmentationImage&& other) = default;

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        SegmentationImage with_draw_order(rerun::components::DrawOrder _draw_order) && {
            draw_order = std::move(_draw_order);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 1;
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
