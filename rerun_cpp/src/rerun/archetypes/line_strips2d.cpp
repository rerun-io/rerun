// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#include "line_strips2d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char LineStrips2D::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.LineStrips2DIndicator";

        std::vector<AnonymousComponentBatch> LineStrips2D::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(7);

            comp_batches.emplace_back(strips);
            if (radii.has_value()) {
                comp_batches.emplace_back(radii.value());
            }
            if (colors.has_value()) {
                comp_batches.emplace_back(colors.value());
            }
            if (labels.has_value()) {
                comp_batches.emplace_back(labels.value());
            }
            if (draw_order.has_value()) {
                comp_batches.emplace_back(draw_order.value());
            }
            if (class_ids.has_value()) {
                comp_batches.emplace_back(class_ids.value());
            }
            if (instance_keys.has_value()) {
                comp_batches.emplace_back(instance_keys.value());
            }
            comp_batches.emplace_back(
                ComponentBatch<
                    components::IndicatorComponent<LineStrips2D::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
