// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visual_bounds2d.fbs".

#include "visual_bounds2d.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    VisualBounds2D VisualBounds2D::clear_fields() {
        auto archetype = VisualBounds2D();
        archetype.range =
            ComponentBatch::empty<rerun::blueprint::components::VisualBounds2D>(Descriptor_range)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> VisualBounds2D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(2);
        if (range.has_value()) {
            columns.push_back(range.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<VisualBounds2D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> VisualBounds2D::columns() {
        if (range.has_value()) {
            return columns(std::vector<uint32_t>(range.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::VisualBounds2D>::serialize(
            const blueprint::archetypes::VisualBounds2D& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(2);

        if (archetype.range.has_value()) {
            cells.push_back(archetype.range.value());
        }
        {
            auto result = ComponentBatch::from_indicator<VisualBounds2D>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
