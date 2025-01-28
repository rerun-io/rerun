// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/geo_points.fbs".

#include "geo_points.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    GeoPoints GeoPoints::clear_fields() {
        auto archetype = GeoPoints();
        archetype.positions =
            ComponentBatch::empty<rerun::components::LatLon>(Descriptor_positions).value_or_throw();
        archetype.radii =
            ComponentBatch::empty<rerun::components::Radius>(Descriptor_radii).value_or_throw();
        archetype.colors =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_colors).value_or_throw();
        archetype.class_ids =
            ComponentBatch::empty<rerun::components::ClassId>(Descriptor_class_ids)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> GeoPoints::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(5);
        if (positions.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(positions.value(), lengths_)
                                  .value_or_throw());
        }
        if (radii.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(radii.value(), lengths_).value_or_throw()
            );
        }
        if (colors.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(colors.value(), lengths_).value_or_throw()
            );
        }
        if (class_ids.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(class_ids.value(), lengths_)
                                  .value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<GeoPoints>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> GeoPoints::columns() {
        if (positions.has_value()) {
            return columns(std::vector<uint32_t>(positions.value().length(), 1));
        }
        if (radii.has_value()) {
            return columns(std::vector<uint32_t>(radii.value().length(), 1));
        }
        if (colors.has_value()) {
            return columns(std::vector<uint32_t>(colors.value().length(), 1));
        }
        if (class_ids.has_value()) {
            return columns(std::vector<uint32_t>(class_ids.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::GeoPoints>::serialize(
        const archetypes::GeoPoints& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.positions.has_value()) {
            cells.push_back(archetype.positions.value());
        }
        if (archetype.radii.has_value()) {
            cells.push_back(archetype.radii.value());
        }
        if (archetype.colors.has_value()) {
            cells.push_back(archetype.colors.value());
        }
        if (archetype.class_ids.has_value()) {
            cells.push_back(archetype.class_ids.value());
        }
        {
            auto result = ComponentBatch::from_indicator<GeoPoints>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
