// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/view_blueprint.fbs".

#include "view_blueprint.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    ViewBlueprint ViewBlueprint::clear_fields() {
        auto archetype = ViewBlueprint();
        archetype.class_identifier = ComponentBatch::empty<rerun::blueprint::components::ViewClass>(
                                         Descriptor_class_identifier
        )
                                         .value_or_throw();
        archetype.display_name =
            ComponentBatch::empty<rerun::components::Name>(Descriptor_display_name)
                .value_or_throw();
        archetype.space_origin =
            ComponentBatch::empty<rerun::blueprint::components::ViewOrigin>(Descriptor_space_origin)
                .value_or_throw();
        archetype.visible =
            ComponentBatch::empty<rerun::blueprint::components::Visible>(Descriptor_visible)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> ViewBlueprint::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(4);
        if (class_identifier.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(class_identifier.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (display_name.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(display_name.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (space_origin.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(space_origin.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (visible.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(visible.value(), lengths_).value_or_throw()
            );
        }
        return columns;
    }

    Collection<ComponentColumn> ViewBlueprint::columns() {
        if (class_identifier.has_value()) {
            return columns(std::vector<uint32_t>(class_identifier.value().length(), 1));
        }
        if (display_name.has_value()) {
            return columns(std::vector<uint32_t>(display_name.value().length(), 1));
        }
        if (space_origin.has_value()) {
            return columns(std::vector<uint32_t>(space_origin.value().length(), 1));
        }
        if (visible.has_value()) {
            return columns(std::vector<uint32_t>(visible.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::ViewBlueprint>::serialize(
            const blueprint::archetypes::ViewBlueprint& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.class_identifier.has_value()) {
            cells.push_back(archetype.class_identifier.value());
        }
        if (archetype.display_name.has_value()) {
            cells.push_back(archetype.display_name.value());
        }
        if (archetype.space_origin.has_value()) {
            cells.push_back(archetype.space_origin.value());
        }
        if (archetype.visible.has_value()) {
            cells.push_back(archetype.visible.value());
        }
        {
            auto indicator = ViewBlueprint::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
