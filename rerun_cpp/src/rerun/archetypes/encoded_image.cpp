// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/encoded_image.fbs".

#include "encoded_image.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    EncodedImage EncodedImage::clear_fields() {
        auto archetype = EncodedImage();
        archetype.blob =
            ComponentBatch::empty<rerun::components::Blob>(Descriptor_blob).value_or_throw();
        archetype.media_type =
            ComponentBatch::empty<rerun::components::MediaType>(Descriptor_media_type)
                .value_or_throw();
        archetype.opacity =
            ComponentBatch::empty<rerun::components::Opacity>(Descriptor_opacity).value_or_throw();
        archetype.draw_order =
            ComponentBatch::empty<rerun::components::DrawOrder>(Descriptor_draw_order)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> EncodedImage::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(5);
        if (blob.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(blob.value(), lengths_).value_or_throw()
            );
        }
        if (media_type.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(media_type.value(), lengths_)
                                  .value_or_throw());
        }
        if (opacity.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(opacity.value(), lengths_).value_or_throw()
            );
        }
        if (draw_order.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(draw_order.value(), lengths_)
                                  .value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<EncodedImage>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> EncodedImage::columns() {
        if (blob.has_value()) {
            return columns(std::vector<uint32_t>(blob.value().length(), 1));
        }
        if (media_type.has_value()) {
            return columns(std::vector<uint32_t>(media_type.value().length(), 1));
        }
        if (opacity.has_value()) {
            return columns(std::vector<uint32_t>(opacity.value().length(), 1));
        }
        if (draw_order.has_value()) {
            return columns(std::vector<uint32_t>(draw_order.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::EncodedImage>::serialize(
        const archetypes::EncodedImage& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.blob.has_value()) {
            cells.push_back(archetype.blob.value());
        }
        if (archetype.media_type.has_value()) {
            cells.push_back(archetype.media_type.value());
        }
        if (archetype.opacity.has_value()) {
            cells.push_back(archetype.opacity.value());
        }
        if (archetype.draw_order.has_value()) {
            cells.push_back(archetype.draw_order.value());
        }
        {
            auto indicator = EncodedImage::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
