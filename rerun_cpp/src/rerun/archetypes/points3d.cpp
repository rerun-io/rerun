// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/archetypes/points3d.fbs"

#include "points3d.hpp"

#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/keypoint_id.hpp"
#include "../components/label.hpp"
#include "../components/point3d.hpp"
#include "../components/radius.hpp"

namespace rerun {
    namespace archetypes {
        Result<std::vector<rerun::DataCell>> Points3D::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(7);

            {
                const auto result =
                    rerun::components::Point3D::to_data_cell(points.data(), points.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (radii.has_value()) {
                const auto& value = radii.value();
                const auto result =
                    rerun::components::Radius::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (colors.has_value()) {
                const auto& value = colors.value();
                const auto result =
                    rerun::components::Color::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (labels.has_value()) {
                const auto& value = labels.value();
                const auto result =
                    rerun::components::Label::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (class_ids.has_value()) {
                const auto& value = class_ids.value();
                const auto result =
                    rerun::components::ClassId::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (keypoint_ids.has_value()) {
                const auto& value = keypoint_ids.value();
                const auto result =
                    rerun::components::KeypointId::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (instance_keys.has_value()) {
                const auto& value = instance_keys.value();
                const auto result =
                    rerun::components::InstanceKey::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = create_indicator_component(
                    "rerun.components.Points3DIndicator",
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
