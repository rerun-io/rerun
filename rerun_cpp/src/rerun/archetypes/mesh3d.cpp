// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#include "mesh3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    Mesh3D Mesh3D::clear_fields() {
        auto archetype = Mesh3D();
        archetype.vertex_positions =
            ComponentBatch::empty<rerun::components::Position3D>(Descriptor_vertex_positions)
                .value_or_throw();
        archetype.triangle_indices =
            ComponentBatch::empty<rerun::components::TriangleIndices>(Descriptor_triangle_indices)
                .value_or_throw();
        archetype.vertex_normals =
            ComponentBatch::empty<rerun::components::Vector3D>(Descriptor_vertex_normals)
                .value_or_throw();
        archetype.vertex_colors =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_vertex_colors)
                .value_or_throw();
        archetype.vertex_texcoords =
            ComponentBatch::empty<rerun::components::Texcoord2D>(Descriptor_vertex_texcoords)
                .value_or_throw();
        archetype.albedo_factor =
            ComponentBatch::empty<rerun::components::AlbedoFactor>(Descriptor_albedo_factor)
                .value_or_throw();
        archetype.albedo_texture_buffer =
            ComponentBatch::empty<rerun::components::ImageBuffer>(Descriptor_albedo_texture_buffer)
                .value_or_throw();
        archetype.albedo_texture_format =
            ComponentBatch::empty<rerun::components::ImageFormat>(Descriptor_albedo_texture_format)
                .value_or_throw();
        archetype.class_ids =
            ComponentBatch::empty<rerun::components::ClassId>(Descriptor_class_ids)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> Mesh3D::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(10);
        if (vertex_positions.has_value()) {
            columns.push_back(vertex_positions.value().partitioned(lengths_).value_or_throw());
        }
        if (triangle_indices.has_value()) {
            columns.push_back(triangle_indices.value().partitioned(lengths_).value_or_throw());
        }
        if (vertex_normals.has_value()) {
            columns.push_back(vertex_normals.value().partitioned(lengths_).value_or_throw());
        }
        if (vertex_colors.has_value()) {
            columns.push_back(vertex_colors.value().partitioned(lengths_).value_or_throw());
        }
        if (vertex_texcoords.has_value()) {
            columns.push_back(vertex_texcoords.value().partitioned(lengths_).value_or_throw());
        }
        if (albedo_factor.has_value()) {
            columns.push_back(albedo_factor.value().partitioned(lengths_).value_or_throw());
        }
        if (albedo_texture_buffer.has_value()) {
            columns.push_back(albedo_texture_buffer.value().partitioned(lengths_).value_or_throw());
        }
        if (albedo_texture_format.has_value()) {
            columns.push_back(albedo_texture_format.value().partitioned(lengths_).value_or_throw());
        }
        if (class_ids.has_value()) {
            columns.push_back(class_ids.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<Mesh3D>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> Mesh3D::columns() {
        if (vertex_positions.has_value()) {
            return columns(std::vector<uint32_t>(vertex_positions.value().length(), 1));
        }
        if (triangle_indices.has_value()) {
            return columns(std::vector<uint32_t>(triangle_indices.value().length(), 1));
        }
        if (vertex_normals.has_value()) {
            return columns(std::vector<uint32_t>(vertex_normals.value().length(), 1));
        }
        if (vertex_colors.has_value()) {
            return columns(std::vector<uint32_t>(vertex_colors.value().length(), 1));
        }
        if (vertex_texcoords.has_value()) {
            return columns(std::vector<uint32_t>(vertex_texcoords.value().length(), 1));
        }
        if (albedo_factor.has_value()) {
            return columns(std::vector<uint32_t>(albedo_factor.value().length(), 1));
        }
        if (albedo_texture_buffer.has_value()) {
            return columns(std::vector<uint32_t>(albedo_texture_buffer.value().length(), 1));
        }
        if (albedo_texture_format.has_value()) {
            return columns(std::vector<uint32_t>(albedo_texture_format.value().length(), 1));
        }
        if (class_ids.has_value()) {
            return columns(std::vector<uint32_t>(class_ids.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Mesh3D>::serialize(
        const archetypes::Mesh3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(10);

        if (archetype.vertex_positions.has_value()) {
            cells.push_back(archetype.vertex_positions.value());
        }
        if (archetype.triangle_indices.has_value()) {
            cells.push_back(archetype.triangle_indices.value());
        }
        if (archetype.vertex_normals.has_value()) {
            cells.push_back(archetype.vertex_normals.value());
        }
        if (archetype.vertex_colors.has_value()) {
            cells.push_back(archetype.vertex_colors.value());
        }
        if (archetype.vertex_texcoords.has_value()) {
            cells.push_back(archetype.vertex_texcoords.value());
        }
        if (archetype.albedo_factor.has_value()) {
            cells.push_back(archetype.albedo_factor.value());
        }
        if (archetype.albedo_texture_buffer.has_value()) {
            cells.push_back(archetype.albedo_texture_buffer.value());
        }
        if (archetype.albedo_texture_format.has_value()) {
            cells.push_back(archetype.albedo_texture_format.value());
        }
        if (archetype.class_ids.has_value()) {
            cells.push_back(archetype.class_ids.value());
        }
        {
            auto result = ComponentBatch::from_indicator<Mesh3D>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
