// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#include "mesh3d.hpp"

namespace rerun {
    namespace archetypes {
        const char Mesh3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Mesh3DIndicator";
    }

    Result<std::vector<SerializedComponentBatch>> AsComponents<archetypes::Mesh3D>::serialize(
        const archetypes::Mesh3D& archetype
    ) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(7);

        {
            auto result = (archetype.vertex_positions).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.mesh_properties.has_value()) {
            auto result =
                ComponentBatch<rerun::components::MeshProperties>(archetype.mesh_properties.value())
                    .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.vertex_normals.has_value()) {
            auto result = (archetype.vertex_normals.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.vertex_colors.has_value()) {
            auto result = (archetype.vertex_colors.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.mesh_material.has_value()) {
            auto result =
                ComponentBatch<rerun::components::Material>(archetype.mesh_material.value())
                    .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = (archetype.class_ids.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.instance_keys.has_value()) {
            auto result = (archetype.instance_keys.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch<Mesh3D::IndicatorComponent>(Mesh3D::IndicatorComponent())
                              .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
