// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/scalar_axis.fbs".

#include "scalar_axis.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    ScalarAxis ScalarAxis::clear_fields() {
        auto archetype = ScalarAxis();
        archetype.range =
            ComponentBatch::empty<rerun::components::Range1D>(Descriptor_range).value_or_throw();
        archetype.zoom_lock =
            ComponentBatch::empty<rerun::blueprint::components::LockRangeDuringZoom>(
                Descriptor_zoom_lock
            )
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> ScalarAxis::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(3);
        if (range.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(range.value(), lengths_).value_or_throw()
            );
        }
        if (zoom_lock.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(zoom_lock.value(), lengths_)
                                  .value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<ScalarAxis>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> ScalarAxis::columns() {
        if (range.has_value()) {
            return columns(std::vector<uint32_t>(range.value().length(), 1));
        }
        if (zoom_lock.has_value()) {
            return columns(std::vector<uint32_t>(zoom_lock.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<blueprint::archetypes::ScalarAxis>::serialize(
        const blueprint::archetypes::ScalarAxis& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        if (archetype.range.has_value()) {
            cells.push_back(archetype.range.value());
        }
        if (archetype.zoom_lock.has_value()) {
            cells.push_back(archetype.zoom_lock.value());
        }
        {
            auto result = ComponentBatch::from_indicator<ScalarAxis>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
