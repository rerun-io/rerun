// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/line_grid3d.fbs".

#include "line_grid3d.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    LineGrid3D LineGrid3D::clear_fields() {
        auto archetype = LineGrid3D();
        archetype.visible =
            ComponentBatch::empty<rerun::blueprint::components::Visible>(Descriptor_visible)
                .value_or_throw();
        archetype.spacing =
            ComponentBatch::empty<rerun::blueprint::components::GridSpacing>(Descriptor_spacing)
                .value_or_throw();
        archetype.plane =
            ComponentBatch::empty<rerun::components::Plane3D>(Descriptor_plane).value_or_throw();
        archetype.stroke_width =
            ComponentBatch::empty<rerun::components::StrokeWidth>(Descriptor_stroke_width)
                .value_or_throw();
        archetype.color =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_color).value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> LineGrid3D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(6);
        if (visible.has_value()) {
            columns.push_back(visible.value().partitioned(lengths_).value_or_throw());
        }
        if (spacing.has_value()) {
            columns.push_back(spacing.value().partitioned(lengths_).value_or_throw());
        }
        if (plane.has_value()) {
            columns.push_back(plane.value().partitioned(lengths_).value_or_throw());
        }
        if (stroke_width.has_value()) {
            columns.push_back(stroke_width.value().partitioned(lengths_).value_or_throw());
        }
        if (color.has_value()) {
            columns.push_back(color.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<LineGrid3D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> LineGrid3D::columns() {
        if (visible.has_value()) {
            return columns(std::vector<uint32_t>(visible.value().length(), 1));
        }
        if (spacing.has_value()) {
            return columns(std::vector<uint32_t>(spacing.value().length(), 1));
        }
        if (plane.has_value()) {
            return columns(std::vector<uint32_t>(plane.value().length(), 1));
        }
        if (stroke_width.has_value()) {
            return columns(std::vector<uint32_t>(stroke_width.value().length(), 1));
        }
        if (color.has_value()) {
            return columns(std::vector<uint32_t>(color.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<Collection<ComponentBatch>> AsComponents<blueprint::archetypes::LineGrid3D>::as_batches(
        const blueprint::archetypes::LineGrid3D& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(6);

        if (archetype.visible.has_value()) {
            cells.push_back(archetype.visible.value());
        }
        if (archetype.spacing.has_value()) {
            cells.push_back(archetype.spacing.value());
        }
        if (archetype.plane.has_value()) {
            cells.push_back(archetype.plane.value());
        }
        if (archetype.stroke_width.has_value()) {
            cells.push_back(archetype.stroke_width.value());
        }
        if (archetype.color.has_value()) {
            cells.push_back(archetype.color.value());
        }
        {
            auto result = ComponentBatch::from_indicator<LineGrid3D>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return rerun::take_ownership(std::move(cells));
    }
} // namespace rerun
