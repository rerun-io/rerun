// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#include "line_strips2d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char LineStrips2D::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.LineStrips2DIndicator";

        std::vector<AnonymousComponentBatch> LineStrips2D::as_component_lists() const {
            std::vector<AnonymousComponentBatch> cells;
            cells.reserve(7);

            cells.emplace_back(strips);
            if (radii.has_value()) {
                cells.emplace_back(radii.value());
            }
            if (colors.has_value()) {
                cells.emplace_back(colors.value());
            }
            if (labels.has_value()) {
                cells.emplace_back(labels.value());
            }
            if (draw_order.has_value()) {
                cells.emplace_back(draw_order.value());
            }
            if (class_ids.has_value()) {
                cells.emplace_back(class_ids.value());
            }
            if (instance_keys.has_value()) {
                cells.emplace_back(instance_keys.value());
            }
            cells.emplace_back(
                ComponentBatch<
                    components::IndicatorComponent<LineStrips2D::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
