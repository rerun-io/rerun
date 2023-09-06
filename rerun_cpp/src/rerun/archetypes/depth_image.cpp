// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#include "depth_image.hpp"

#include "../components/depth_meter.hpp"
#include "../components/draw_order.hpp"
#include "../components/tensor_data.hpp"

namespace rerun {
    namespace archetypes {
        Result<std::vector<rerun::DataCell>> DepthImage::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(3);

            {
                const auto result = rerun::components::TensorData::to_data_cell(&data, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (meter.has_value()) {
                const auto& value = meter.value();
                const auto result = rerun::components::DepthMeter::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (draw_order.has_value()) {
                const auto& value = draw_order.value();
                const auto result = rerun::components::DrawOrder::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = create_indicator_component(
                    "rerun.components.DepthImageIndicator",
                    num_instances()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
