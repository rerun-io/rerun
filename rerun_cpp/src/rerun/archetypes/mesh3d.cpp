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
        cells.reserve(8);

        {
            auto result =
                DataCell::from_loggable<rerun::components::Position3D>(archetype.vertex_positions);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.mesh_properties.has_value()) {
            auto result = DataCell::from_loggable<rerun::components::MeshProperties>(
                archetype.mesh_properties.value()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.vertex_normals.has_value()) {
            auto result = DataCell::from_loggable<rerun::components::Vector3D>(
                archetype.vertex_normals.value()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.vertex_colors.has_value()) {
            auto result =
                DataCell::from_loggable<rerun::components::Color>(archetype.vertex_colors.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.mesh_material.has_value()) {
            auto result =
                DataCell::from_loggable<rerun::components::Material>(archetype.mesh_material.value()
                );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result =
                DataCell::from_loggable<rerun::components::ClassId>(archetype.class_ids.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.instance_keys.has_value()) {
            auto result = DataCell::from_loggable<rerun::components::InstanceKey>(
                archetype.instance_keys.value()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Mesh3D::IndicatorComponent();
            auto result = DataCell::from_loggable<decltype(indicator)>(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
