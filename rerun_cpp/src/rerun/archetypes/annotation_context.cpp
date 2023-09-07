// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

#include "annotation_context.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char AnnotationContext::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.AnnotationContextIndicator";

        std::vector<AnonymousComponentList> AnnotationContext::as_component_lists() const {
            std::vector<AnonymousComponentList> cells;
            cells.reserve(1);

            cells.emplace_back(context);
            cells.emplace_back(
                ComponentList<
                    components::IndicatorComponent<AnnotationContext::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
