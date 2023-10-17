// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/image.fbs".

#pragma once

#include "../component_batch.hpp"
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
        /// **Archetype**: A monochrome or color image.
        ///
        /// The shape of the `TensorData` must be mappable to:
        /// - A `HxW` tensor, treated as a grayscale image.
        /// - A `HxWx3` tensor, treated as an RGB image.
        /// - A `HxWx4` tensor, treated as an RGBA image.
        ///
        /// Leading and trailing unit-dimensions are ignored, so that
        /// `1x640x480x3x1` is treated as a `640x480x3` RGB image.
        ///
        /// ## Example
        ///
        /// ### `image_simple`:
        /// ```cpp,ignore
        /// #include <rerun.hpp>
        ///
        /// int main() {
        ///     auto rec = rerun::RecordingStream("rerun_example_image_simple");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     // Create a synthetic image.
        ///     const int HEIGHT = 8;
        ///     const int WIDTH = 12;
        ///     std::vector<uint8_t> data(WIDTH * HEIGHT * 3, 0);
        ///     for (auto i = 0; i <data.size(); i += 3) {
        ///         data[i] = 255;
        ///     }
        ///     for (auto y = 0; y <4; ++y) { // top half
        ///         auto row = data.begin() + y * WIDTH * 3;
        ///         for (auto i = 0; i <6 * 3; i += 3) { // left half
        ///             row[i] = 0;
        ///             row[i + 1] = 255;
        ///         }
        ///     }
        ///
        ///     rec.log("image", rerun::Image(rerun::TensorData({HEIGHT, WIDTH, 3}, std::move(data))));
        /// }
        /// ```
        struct Image {
            /// The image data. Should always be a rank-2 or rank-3 tensor.
            rerun::components::TensorData data;

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            std::optional<rerun::components::DrawOrder> draw_order;

            /// Name of the indicator component, used to identify the archetype when converting to a list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            /// Indicator component, used to identify the archetype when converting to a list of components.
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            // Extensions to generated type defined in 'image_ext.cpp'

            /// New image from tensor data.
            ///
            /// Sets dimensions to width/height if they are not specified.
            /// Calls Error::handle() if the shape is not rank 2.
            explicit Image(rerun::components::TensorData _data);

          public:
            Image() = default;
            Image(Image&& other) = default;

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            Image with_draw_order(rerun::components::DrawOrder _draw_order) && {
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
    struct AsComponents<archetypes::Image> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::Image& archetype
        );
    };
} // namespace rerun
