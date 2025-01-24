// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/view_coordinates.fbs".

#include "view_coordinates.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    ViewCoordinates ViewCoordinates::clear_fields() {
        auto archetype = ViewCoordinates();
        archetype.xyz = ComponentBatch::empty<rerun::components::ViewCoordinates>(Descriptor_xyz)
                            .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> ViewCoordinates::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(1);
        if (xyz.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(xyz.value(), lengths_).value_or_throw()
            );
        }
        return columns;
    }

    Collection<ComponentColumn> ViewCoordinates::columns() {
        if (xyz.has_value()) {
            return columns(std::vector<uint32_t>(xyz.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::ViewCoordinates>::serialize(
        const archetypes::ViewCoordinates& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(2);

        if (archetype.xyz.has_value()) {
            cells.push_back(archetype.xyz.value());
        }
        {
            auto indicator = ViewCoordinates::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
