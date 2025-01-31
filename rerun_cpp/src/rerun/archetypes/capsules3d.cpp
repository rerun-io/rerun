// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/capsules3d.fbs".

#include "capsules3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    Capsules3D Capsules3D::clear_fields() {
        auto archetype = Capsules3D();
        archetype.lengths =
            ComponentBatch::empty<rerun::components::Length>(Descriptor_lengths).value_or_throw();
        archetype.radii =
            ComponentBatch::empty<rerun::components::Radius>(Descriptor_radii).value_or_throw();
        archetype.translations =
            ComponentBatch::empty<rerun::components::PoseTranslation3D>(Descriptor_translations)
                .value_or_throw();
        archetype.rotation_axis_angles =
            ComponentBatch::empty<rerun::components::PoseRotationAxisAngle>(
                Descriptor_rotation_axis_angles
            )
                .value_or_throw();
        archetype.quaternions =
            ComponentBatch::empty<rerun::components::PoseRotationQuat>(Descriptor_quaternions)
                .value_or_throw();
        archetype.colors =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_colors).value_or_throw();
        archetype.labels =
            ComponentBatch::empty<rerun::components::Text>(Descriptor_labels).value_or_throw();
        archetype.show_labels =
            ComponentBatch::empty<rerun::components::ShowLabels>(Descriptor_show_labels)
                .value_or_throw();
        archetype.class_ids =
            ComponentBatch::empty<rerun::components::ClassId>(Descriptor_class_ids)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> Capsules3D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(10);
        if (lengths.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(lengths.value(), lengths_).value_or_throw()
            );
        }
        if (radii.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(radii.value(), lengths_).value_or_throw()
            );
        }
        if (translations.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(translations.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (rotation_axis_angles.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(rotation_axis_angles.value(), lengths_)
                    .value_or_throw()
            );
        }
        if (quaternions.has_value()) {
            columns.push_back(
                ComponentColumn::from_batch_with_lengths(quaternions.value(), lengths_)
                    .value_or_throw()
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
        if (class_ids.has_value()) {
            columns.push_back(ComponentColumn::from_batch_with_lengths(class_ids.value(), lengths_)
                                  .value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<Capsules3D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> Capsules3D::columns() {
        if (lengths.has_value()) {
            return columns(std::vector<uint32_t>(lengths.value().length(), 1));
        }
        if (radii.has_value()) {
            return columns(std::vector<uint32_t>(radii.value().length(), 1));
        }
        if (translations.has_value()) {
            return columns(std::vector<uint32_t>(translations.value().length(), 1));
        }
        if (rotation_axis_angles.has_value()) {
            return columns(std::vector<uint32_t>(rotation_axis_angles.value().length(), 1));
        }
        if (quaternions.has_value()) {
            return columns(std::vector<uint32_t>(quaternions.value().length(), 1));
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
        if (class_ids.has_value()) {
            return columns(std::vector<uint32_t>(class_ids.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Capsules3D>::serialize(
        const archetypes::Capsules3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(10);

        if (archetype.lengths.has_value()) {
            cells.push_back(archetype.lengths.value());
        }
        if (archetype.radii.has_value()) {
            cells.push_back(archetype.radii.value());
        }
        if (archetype.translations.has_value()) {
            cells.push_back(archetype.translations.value());
        }
        if (archetype.rotation_axis_angles.has_value()) {
            cells.push_back(archetype.rotation_axis_angles.value());
        }
        if (archetype.quaternions.has_value()) {
            cells.push_back(archetype.quaternions.value());
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
        if (archetype.class_ids.has_value()) {
            cells.push_back(archetype.class_ids.value());
        }
        {
            auto result = ComponentBatch::from_indicator<Capsules3D>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
