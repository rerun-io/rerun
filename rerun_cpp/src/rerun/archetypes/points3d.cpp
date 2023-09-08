// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs".

#include "points3d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Points3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Points3DIndicator";

        std::vector<AnonymousComponentBatch> Points3D::as_component_lists() const {
            std::vector<AnonymousComponentBatch> cells;
            cells.reserve(7);

            cells.emplace_back(points);
            if (radii.has_value()) {
                cells.emplace_back(radii.value());
            }
            if (colors.has_value()) {
                cells.emplace_back(colors.value());
            }
            if (labels.has_value()) {
                cells.emplace_back(labels.value());
            }
            if (class_ids.has_value()) {
                cells.emplace_back(class_ids.value());
            }
            if (keypoint_ids.has_value()) {
                cells.emplace_back(keypoint_ids.value());
            }
            if (instance_keys.has_value()) {
                cells.emplace_back(instance_keys.value());
            }
            cells.emplace_back(
                ComponentBatch<components::IndicatorComponent<Points3D::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
