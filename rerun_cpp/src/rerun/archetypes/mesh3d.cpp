// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#include "mesh3d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Mesh3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Mesh3DIndicator";

        std::vector<AnonymousComponentBatch> Mesh3D::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(7);

            comp_batches.emplace_back(vertex_positions);
            if (mesh_properties.has_value()) {
                comp_batches.emplace_back(mesh_properties.value());
            }
            if (vertex_normals.has_value()) {
                comp_batches.emplace_back(vertex_normals.value());
            }
            if (vertex_colors.has_value()) {
                comp_batches.emplace_back(vertex_colors.value());
            }
            if (mesh_material.has_value()) {
                comp_batches.emplace_back(mesh_material.value());
            }
            if (class_ids.has_value()) {
                comp_batches.emplace_back(class_ids.value());
            }
            if (instance_keys.has_value()) {
                comp_batches.emplace_back(instance_keys.value());
            }
            comp_batches.emplace_back(
                ComponentBatch<components::IndicatorComponent<Mesh3D::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
