// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/arrows3d.fbs"

#include "arrows3d.hpp"

#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/label.hpp"
#include "../components/origin3d.hpp"
#include "../components/radius.hpp"
#include "../components/vector3d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace archetypes {
        Result<std::vector<rerun::DataCell>> Arrows3D::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(7);

            {
                const auto result =
                    rerun::components::Vector3D::to_data_cell(vectors.data(), vectors.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (origins.has_value()) {
                const auto& value = origins.value();
                const auto result =
                    rerun::components::Origin3D::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (radii.has_value()) {
                const auto& value = radii.value();
                const auto result =
                    rerun::components::Radius::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (colors.has_value()) {
                const auto& value = colors.value();
                const auto result =
                    rerun::components::Color::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (labels.has_value()) {
                const auto& value = labels.value();
                const auto result =
                    rerun::components::Label::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (class_ids.has_value()) {
                const auto& value = class_ids.value();
                const auto result =
                    rerun::components::ClassId::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }
            if (instance_keys.has_value()) {
                const auto& value = instance_keys.value();
                const auto result =
                    rerun::components::InstanceKey::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.push_back(result.value);
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
