// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs"

#include "points3d.hpp"

#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/keypoint_id.hpp"
#include "../components/label.hpp"
#include "../components/point3d.hpp"
#include "../components/radius.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace archetypes {
        arrow::Result<std::vector<rerun::DataCell>> Points3D::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(7);

            {
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::Point3D::to_data_cell(points.data(), points.size())
                );
                cells.push_back(cell);
            }
            if (radii.has_value()) {
                const auto& value = radii.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::Radius::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }
            if (colors.has_value()) {
                const auto& value = colors.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::Color::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }
            if (labels.has_value()) {
                const auto& value = labels.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::Label::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }
            if (class_ids.has_value()) {
                const auto& value = class_ids.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::ClassId::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }
            if (keypoint_ids.has_value()) {
                const auto& value = keypoint_ids.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::KeypointId::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }
            if (instance_keys.has_value()) {
                const auto& value = instance_keys.value();
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::InstanceKey::to_data_cell(value.data(), value.size())
                );
                cells.push_back(cell);
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
