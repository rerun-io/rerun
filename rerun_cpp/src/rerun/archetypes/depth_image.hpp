// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#pragma once

#include "../component_batch.hpp"
#include "../components/depth_meter.hpp"
#include "../components/draw_order.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"
#include "../warning_macros.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: A depth image.
        ///
        /// The shape of the `TensorData` must be mappable to an `HxW` tensor.
        /// Each pixel corresponds to a depth value in units specified by `meter`.
        ///
        /// ## Example
        ///
        /// ### Depth to 3D example
        /// ![image](https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/full.png)
        ///
        /// ```cpp
        /// #include <rerun.hpp>
        ///
        /// #include <algorithm> // fill_n
        /// #include <vector>
        ///
        /// int main() {
        ///     const auto rec = rerun::RecordingStream("rerun_example_depth_image");
        ///     rec.spawn().exit_on_failure();
        ///
        ///     // Create a synthetic depth image.
        ///     const int HEIGHT = 200;
        ///     const int WIDTH = 300;
        ///     std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
        ///     for (auto y = 50; y <150; ++y) {
        ///         std::fill_n(data.begin() + y * WIDTH + 50, 100, static_cast<uint16_t>(20000));
        ///     }
        ///     for (auto y = 130; y <180; ++y) {
        ///         std::fill_n(data.begin() + y * WIDTH + 100, 180, static_cast<uint16_t>(45000));
        ///     }
        ///
        ///     // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
        ///     rec.log(
        ///         "world/camera",
        ///         rerun::Pinhole::from_focal_length_and_resolution(
        ///             200.0f,
        ///             {static_cast<float>(WIDTH), static_cast<float>(HEIGHT)}
        ///         )
        ///     );
        ///
        ///     rec.log(
        ///         "world/camera/depth",
        ///         rerun::DepthImage({HEIGHT, WIDTH}, std::move(data)).with_meter(10000.0)
        ///     );
        /// }
        /// ```
        struct DepthImage {
            /// The depth-image data. Should always be a rank-2 tensor.
            rerun::components::TensorData data;

            /// An optional floating point value that specifies how long a meter is in the native depth units.
            ///
            /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
            /// and a range of up to ~65 meters (2^16 / 1000).
            std::optional<rerun::components::DepthMeter> meter;

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            std::optional<rerun::components::DrawOrder> draw_order;

            /// Name of the indicator component, used to identify the archetype when converting to a list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            /// Indicator component, used to identify the archetype when converting to a list of components.
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            // Extensions to generated type defined in 'depth_image_ext.cpp'

            /// New depth image from height/width and tensor buffer.
            ///
            /// Sets the dimension names to "height" and "width" if they are not specified.
            /// Calls `Error::handle()` if the shape is not rank 2.
            DepthImage(
                std::vector<datatypes::TensorDimension> shape, datatypes::TensorBuffer buffer
            )
                : DepthImage(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

            /// New depth image from tensor data.
            ///
            /// Sets the dimension names to "height" and "width" if they are not specified.
            /// Calls `Error::handle()` if the shape is not rank 2.
            explicit DepthImage(components::TensorData _data);

          public:
            DepthImage() = default;
            DepthImage(DepthImage&& other) = default;

            /// An optional floating point value that specifies how long a meter is in the native depth units.
            ///
            /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
            /// and a range of up to ~65 meters (2^16 / 1000).
            DepthImage with_meter(rerun::components::DepthMeter _meter) && {
                meter = std::move(_meter);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            DepthImage with_draw_order(rerun::components::DrawOrder _draw_order) && {
                draw_order = std::move(_draw_order);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }
        };

    } // namespace archetypes

    /// \private
    template <typename T>
    struct AsComponents; /// \private

    template <>
    struct AsComponents<archetypes::DepthImage> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::DepthImage& archetype
        );
    };
} // namespace rerun
