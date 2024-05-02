// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#include "mesh3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::Mesh3D>::serialize(
        const archetypes::Mesh3D& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(9);

        {
            auto result = DataCell::from_loggable(archetype.vertex_positions);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.triangle_indices.has_value()) {
            auto result = DataCell::from_loggable(archetype.triangle_indices.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.vertex_normals.has_value()) {
            auto result = DataCell::from_loggable(archetype.vertex_normals.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.vertex_colors.has_value()) {
            auto result = DataCell::from_loggable(archetype.vertex_colors.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.vertex_texcoords.has_value()) {
            auto result = DataCell::from_loggable(archetype.vertex_texcoords.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.mesh_material.has_value()) {
            auto result = DataCell::from_loggable(archetype.mesh_material.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.albedo_texture.has_value()) {
            auto result = DataCell::from_loggable(archetype.albedo_texture.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = DataCell::from_loggable(archetype.class_ids.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Mesh3D::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
