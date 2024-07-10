// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/material.hpp"
#include "../components/position3d.hpp"
#include "../components/tensor_data.hpp"
#include "../components/texcoord2d.hpp"
#include "../components/triangle_indices.hpp"
#include "../components/vector3d.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A 3D triangle mesh as specified by its per-mesh and per-vertex properties.
    ///
    /// ## Example
    ///
    /// ### Simple indexed 3D mesh
    /// ![image](https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_mesh3d_indexed");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     const rerun::Position3D vertex_positions[3] = {
    ///         {0.0f, 1.0f, 0.0f},
    ///         {1.0f, 0.0f, 0.0f},
    ///         {0.0f, 0.0f, 0.0f},
    ///     };
    ///     const rerun::Color vertex_colors[3] = {
    ///         {0, 0, 255},
    ///         {0, 255, 0},
    ///         {255, 0, 0},
    ///     };
    ///
    ///     rec.log(
    ///         "triangle",
    ///         rerun::Mesh3D(vertex_positions)
    ///             .with_vertex_normals({{0.0, 0.0, 1.0}})
    ///             .with_vertex_colors(vertex_colors)
    ///             .with_triangle_indices({{2, 1, 0}})
    ///     );
    /// }
    /// ```
    struct Mesh3D {
        /// The positions of each vertex.
        ///
        /// If no `triangle_indices` are specified, then each triplet of positions is interpreted as a triangle.
        Collection<rerun::components::Position3D> vertex_positions;

        /// Optional indices for the triangles that make up the mesh.
        std::optional<Collection<rerun::components::TriangleIndices>> triangle_indices;

        /// An optional normal for each vertex.
        std::optional<Collection<rerun::components::Vector3D>> vertex_normals;

        /// An optional color for each vertex.
        std::optional<Collection<rerun::components::Color>> vertex_colors;

        /// An optional uv texture coordinate for each vertex.
        std::optional<Collection<rerun::components::Texcoord2D>> vertex_texcoords;

        /// Optional material properties for the mesh as a whole.
        std::optional<rerun::components::Material> mesh_material;

        /// Optional albedo texture.
        ///
        /// Used with `vertex_texcoords` on `Mesh3D`.
        /// Currently supports only sRGB(A) textures, ignoring alpha.
        /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        std::optional<rerun::components::TensorData> albedo_texture;

        /// Optional class Ids for the vertices.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        std::optional<Collection<rerun::components::ClassId>> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Mesh3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        Mesh3D() = default;
        Mesh3D(Mesh3D&& other) = default;

        explicit Mesh3D(Collection<rerun::components::Position3D> _vertex_positions)
            : vertex_positions(std::move(_vertex_positions)) {}

        /// Optional indices for the triangles that make up the mesh.
        Mesh3D with_triangle_indices(
            Collection<rerun::components::TriangleIndices> _triangle_indices
        ) && {
            triangle_indices = std::move(_triangle_indices);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional normal for each vertex.
        Mesh3D with_vertex_normals(Collection<rerun::components::Vector3D> _vertex_normals) && {
            vertex_normals = std::move(_vertex_normals);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional color for each vertex.
        Mesh3D with_vertex_colors(Collection<rerun::components::Color> _vertex_colors) && {
            vertex_colors = std::move(_vertex_colors);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional uv texture coordinate for each vertex.
        Mesh3D with_vertex_texcoords(Collection<rerun::components::Texcoord2D> _vertex_texcoords
        ) && {
            vertex_texcoords = std::move(_vertex_texcoords);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional material properties for the mesh as a whole.
        Mesh3D with_mesh_material(rerun::components::Material _mesh_material) && {
            mesh_material = std::move(_mesh_material);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional albedo texture.
        ///
        /// Used with `vertex_texcoords` on `Mesh3D`.
        /// Currently supports only sRGB(A) textures, ignoring alpha.
        /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        Mesh3D with_albedo_texture(rerun::components::TensorData _albedo_texture) && {
            albedo_texture = std::move(_albedo_texture);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional class Ids for the vertices.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        Mesh3D with_class_ids(Collection<rerun::components::ClassId> _class_ids) && {
            class_ids = std::move(_class_ids);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Mesh3D> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::Mesh3D& archetype);
    };
} // namespace rerun
