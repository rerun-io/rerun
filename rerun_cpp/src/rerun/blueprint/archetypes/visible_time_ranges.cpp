// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visible_time_ranges.fbs".

#include "visible_time_ranges.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    VisibleTimeRanges VisibleTimeRanges::clear_fields() {
        auto archetype = VisibleTimeRanges();
        archetype.ranges =
            ComponentBatch::empty<rerun::blueprint::components::VisibleTimeRange>(Descriptor_ranges)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> VisibleTimeRanges::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(1);
        if (ranges.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(ranges.value(), lengths_).value_or_throw()
            );
        }
        return columns;
    }

    Collection<ComponentColumn> VisibleTimeRanges::columns() {
        if (ranges.has_value()) {
            return columns(std::vector<uint32_t>(ranges.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::VisibleTimeRanges>::serialize(
            const blueprint::archetypes::VisibleTimeRanges& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(2);

        if (archetype.ranges.has_value()) {
            cells.push_back(archetype.ranges.value());
        }
        {
            auto indicator = VisibleTimeRanges::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
