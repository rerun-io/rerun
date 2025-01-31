// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/points2d.fbs".

#include "points2d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    Points2D Points2D::clear_fields() {
        auto archetype = Points2D();
        archetype.positions =
            ComponentBatch::empty<rerun::components::Position2D>(Descriptor_positions)
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
        archetype.draw_order =
            ComponentBatch::empty<rerun::components::DrawOrder>(Descriptor_draw_order)
                .value_or_throw();
        archetype.class_ids =
            ComponentBatch::empty<rerun::components::ClassId>(Descriptor_class_ids)
                .value_or_throw();
        archetype.keypoint_ids =
            ComponentBatch::empty<rerun::components::KeypointId>(Descriptor_keypoint_ids)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> Points2D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(9);
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
        if (labels.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(labels.value(), lengths_).value_or_throw()
            );
        }
        if (show_labels.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(show_labels.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (draw_order.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(draw_order.value(), lengths_)
                                  .value_or_throw());
        }
        if (class_ids.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(class_ids.value(), lengths_)
                                  .value_or_throw());
        }
        if (keypoint_ids.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(keypoint_ids.value(), lengths_)
                    .value_or_throw()
            );
        }
        columns.push_back(
            ComponentColumn::from_indicators<Points2D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> Points2D::columns() {
        if (positions.has_value()) {
            return columns(std::vector<uint32_t>(positions.value().length(), 1));
        }
        if (radii.has_value()) {
            return columns(std::vector<uint32_t>(radii.value().length(), 1));
        }
        if (colors.has_value()) {
            return columns(std::vector<uint32_t>(colors.value().length(), 1));
        }
        if (labels.has_value()) {
            return columns(std::vector<uint32_t>(labels.value().length(), 1));
        }
        if (show_labels.has_value()) {
            return columns(std::vector<uint32_t>(show_labels.value().length(), 1));
        }
        if (draw_order.has_value()) {
            return columns(std::vector<uint32_t>(draw_order.value().length(), 1));
        }
        if (class_ids.has_value()) {
            return columns(std::vector<uint32_t>(class_ids.value().length(), 1));
        }
        if (keypoint_ids.has_value()) {
            return columns(std::vector<uint32_t>(keypoint_ids.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<Collection<ComponentBatch>> AsComponents<archetypes::Points2D>::as_batches(
        const archetypes::Points2D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(9);

        if (archetype.positions.has_value()) {
            cells.push_back(archetype.positions.value());
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
        if (archetype.draw_order.has_value()) {
            cells.push_back(archetype.draw_order.value());
        }
        if (archetype.class_ids.has_value()) {
            cells.push_back(archetype.class_ids.value());
        }
        if (archetype.keypoint_ids.has_value()) {
            cells.push_back(archetype.keypoint_ids.value());
        }
        {
            auto result = ComponentBatch::from_indicator<Points2D>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return rerun::take_ownership(std::move(cells));
    }
} // namespace rerun
