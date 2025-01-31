// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_point.fbs".

#include "series_point.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    SeriesPoint SeriesPoint::clear_fields() {
        auto archetype = SeriesPoint();
        archetype.color =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_color).value_or_throw();
        archetype.marker = ComponentBatch::empty<rerun::components::MarkerShape>(Descriptor_marker)
                               .value_or_throw();
        archetype.name =
            ComponentBatch::empty<rerun::components::Name>(Descriptor_name).value_or_throw();
        archetype.marker_size =
            ComponentBatch::empty<rerun::components::MarkerSize>(Descriptor_marker_size)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> SeriesPoint::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(5);
        if (color.has_value()) {
            columns.push_back(color.value().partitioned(lengths_).value_or_throw());
        }
        if (marker.has_value()) {
            columns.push_back(marker.value().partitioned(lengths_).value_or_throw());
        }
        if (name.has_value()) {
            columns.push_back(name.value().partitioned(lengths_).value_or_throw());
        }
        if (marker_size.has_value()) {
            columns.push_back(marker_size.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<SeriesPoint>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> SeriesPoint::columns() {
        if (color.has_value()) {
            return columns(std::vector<uint32_t>(color.value().length(), 1));
        }
        if (marker.has_value()) {
            return columns(std::vector<uint32_t>(marker.value().length(), 1));
        }
        if (name.has_value()) {
            return columns(std::vector<uint32_t>(name.value().length(), 1));
        }
        if (marker_size.has_value()) {
            return columns(std::vector<uint32_t>(marker_size.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::SeriesPoint>::serialize(
        const archetypes::SeriesPoint& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.color.has_value()) {
            cells.push_back(archetype.color.value());
        }
        if (archetype.marker.has_value()) {
            cells.push_back(archetype.marker.value());
        }
        if (archetype.name.has_value()) {
            cells.push_back(archetype.name.value());
        }
        if (archetype.marker_size.has_value()) {
            cells.push_back(archetype.marker_size.value());
        }
        {
            auto result = ComponentBatch::from_indicator<SeriesPoint>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
