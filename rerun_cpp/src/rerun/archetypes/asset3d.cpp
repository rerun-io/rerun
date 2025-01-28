// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/asset3d.fbs".

#include "asset3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    Asset3D Asset3D::clear_fields() {
        auto archetype = Asset3D();
        archetype.blob =
            ComponentBatch::empty<rerun::components::Blob>(Descriptor_blob).value_or_throw();
        archetype.media_type =
            ComponentBatch::empty<rerun::components::MediaType>(Descriptor_media_type)
                .value_or_throw();
        archetype.albedo_factor =
            ComponentBatch::empty<rerun::components::AlbedoFactor>(Descriptor_albedo_factor)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> Asset3D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(4);
        if (blob.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(blob.value(), lengths_).value_or_throw()
            );
        }
        if (media_type.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(media_type.value(), lengths_)
                                  .value_or_throw());
        }
        if (albedo_factor.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(albedo_factor.value(), lengths_)
                    .value_or_throw()
            );
        }
        columns.push_back(
            ComponentColumn::from_indicators<Asset3D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> Asset3D::columns() {
        if (blob.has_value()) {
            return columns(std::vector<uint32_t>(blob.value().length(), 1));
        }
        if (media_type.has_value()) {
            return columns(std::vector<uint32_t>(media_type.value().length(), 1));
        }
        if (albedo_factor.has_value()) {
            return columns(std::vector<uint32_t>(albedo_factor.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Asset3D>::serialize(
        const archetypes::Asset3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(4);

        if (archetype.blob.has_value()) {
            cells.push_back(archetype.blob.value());
        }
        if (archetype.media_type.has_value()) {
            cells.push_back(archetype.media_type.value());
        }
        if (archetype.albedo_factor.has_value()) {
            cells.push_back(archetype.albedo_factor.value());
        }
        {
            auto indicator = Asset3D::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
