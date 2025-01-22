// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/line_strips3d.fbs".

#include "line_strips3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    LineStrips3D LineStrips3D::clear_fields() {
        auto archetype = LineStrips3D();
        archetype.strips = ComponentBatch::empty<rerun::components::LineStrip3D>(Descriptor_strips)
                               .value_or_throw();
        archetype.radii =
            ComponentBatch::empty<rerun::components::Radius>(Descriptor_radii).value_or_throw();
        archetype.colors =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_colors).value_or_throw();
        archetype.labels =
            ComponentBatch::empty<rerun::components::Text>(Descriptor_labels).value_or_throw();
        archetype.show_labels =
            ComponentBatch::empty<rerun::components::ShowLabels>(Descriptor_show_labels)
                .value_or_throw();
        archetype.class_ids =
            ComponentBatch::empty<rerun::components::ClassId>(Descriptor_class_ids)
                .value_or_throw();
        return archetype;
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::LineStrips3D>::serialize(
        const archetypes::LineStrips3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(7);

        if (archetype.strips.has_value()) {
            cells.push_back(archetype.strips.value());
        }
        if (archetype.radii.has_value()) {
            cells.push_back(archetype.radii.value());
        }
        if (archetype.colors.has_value()) {
            cells.push_back(archetype.colors.value());
        }
        if (archetype.labels.has_value()) {
            cells.push_back(archetype.labels.value());
        }
        if (archetype.show_labels.has_value()) {
            cells.push_back(archetype.show_labels.value());
        }
        if (archetype.class_ids.has_value()) {
            cells.push_back(archetype.class_ids.value());
        }
        {
            auto indicator = LineStrips3D::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
