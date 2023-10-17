// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#pragma once

#include "../component_batch.hpp"
#include "../components/depth_meter.hpp"
#include "../components/draw_order.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../error.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

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
        /// ```cpp,ignore
        /// #include <rerun.hpp>
        ///
        /// #include <algorithm>
        ///
        /// int main() {
        ///     auto rec = rerun::RecordingStream("rerun_example_depth_image");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     // Create a synthetic depth image.
        ///     const int HEIGHT = 8;
        ///     const int WIDTH = 12;
        ///     std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
        ///     for (auto y = 0; y <4; ++y) {                       // top half
        ///         std::fill_n(data.begin() + y * WIDTH, 6, 20000); // left half
        ///     }
        ///     for (auto y = 4; y <8; ++y) {                           // bottom half
        ///         std::fill_n(data.begin() + y * WIDTH + 6, 6, 45000); // right half
        ///     }
        ///
        ///     // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
        ///     rec.log(
        ///         "world/camera",
        ///         rerun::Pinhole::focal_length_and_resolution(
        ///             {20.0f, 20.0f},
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

            /// New DepthImage from dimensions and tensor buffer.
            ///
            /// Sets dimensions to width/height if they are not specified.
            /// Calls Error::handle() if the shape is not rank 2.
            DepthImage(
                std::vector<rerun::datatypes::TensorDimension> shape,
                rerun::datatypes::TensorBuffer buffer
            )
                : DepthImage(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

            /// New depth image from tensor data.
            ///
            /// Sets dimensions to width/height if they are not specified.
            /// Calls Error::handle() if the shape is not rank 2.
            explicit DepthImage(rerun::components::TensorData _data);

          public:
            DepthImage() = default;
            DepthImage(DepthImage&& other) = default;

            /// An optional floating point value that specifies how long a meter is in the native depth units.
            ///
            /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
            /// and a range of up to ~65 meters (2^16 / 1000).
            DepthImage with_meter(rerun::components::DepthMeter _meter) && {
                meter = std::move(_meter);
                return std::move(*this);
            }

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            DepthImage with_draw_order(rerun::components::DrawOrder _draw_order) && {
                draw_order = std::move(_draw_order);
                return std::move(*this);
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }
        };

    } // namespace archetypes

    template <typename T>
    struct AsComponents;

    template <>
    struct AsComponents<archetypes::DepthImage> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::DepthImage& archetype
        );
    };
} // namespace rerun
