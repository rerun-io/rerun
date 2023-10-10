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
        struct DepthImage {
            /// The depth-image data. Should always be a rank-2 tensor.
            rerun::components::TensorData data;

            /// An optional floating point value that specifies how long a meter is in the native
            /// depth units.
            ///
            /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter
            /// precision and a range of up to ~65 meters (2^16 / 1000).
            std::optional<rerun::components::DepthMeter> meter;

            /// An optional floating point value that specifies the 2D drawing order.
            ///
            /// Objects with higher values are drawn on top of those with lower values.
            std::optional<rerun::components::DrawOrder> draw_order;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            DepthImage() = default;
            DepthImage(DepthImage&& other) = default;

            DepthImage(rerun::components::TensorData _data) : data(std::move(_data)) {}

            /// An optional floating point value that specifies how long a meter is in the native
            /// depth units.
            ///
            /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter
            /// precision and a range of up to ~65 meters (2^16 / 1000).
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
