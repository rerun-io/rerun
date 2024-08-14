// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/mesh3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/albedo_factor.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/position3d.hpp"
#include "../components/tensor_data.hpp"
#include "../components/texcoord2d.hpp"
#include "../components/triangle_indices.hpp"
#include "../components/vector3d.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A 3D triangle mesh as specified by its per-mesh and per-vertex properties.
    ///
    /// See also `archetypes::Asset3D`.
    ///
    /// If there are multiple `archetypes::InstancePoses3D` instances logged to the same entity as a mesh,
    /// an instance of the mesh will be drawn for each transform.
    ///
    /// ## Examples
    ///
    /// ### Simple indexed 3D mesh
    /// ![image](https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/full.png)
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
    ///
    /// ### 3D mesh with instancing
    /// ![image](https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_mesh3d_instancing");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.set_time_sequence("frame", 0);
    ///     rec.log(
    ///         "shape",
    ///         rerun::Mesh3D(
    ///             {{1.0f, 1.0f, 1.0f}, {-1.0f, -1.0f, 1.0f}, {-1.0f, 1.0f, -1.0f}, {1.0f, -1.0f, -1.0f}}
    ///         )
    ///             .with_triangle_indices({{0, 1, 2}, {0, 1, 3}, {0, 2, 3}, {1, 2, 3}})
    ///             .with_vertex_colors({0xFF0000FF, 0x00FF00FF, 0x00000FFFF, 0xFFFF00FF})
    ///     );
    ///     // This box will not be affected by its parent's instance poses!
    ///     rec.log("shape/box", rerun::Boxes3D::from_half_sizes({{5.0f, 5.0f, 5.0f}}));
    ///
    ///     for (int i = 0; i <100; ++i) {
    ///         rec.set_time_sequence("frame", i);
    ///         rec.log(
    ///             "shape",
    ///             rerun::InstancePoses3D()
    ///                 .with_translations(
    ///                     {{2.0f, 0.0f, 0.0f},
    ///                      {0.0f, 2.0f, 0.0f},
    ///                      {0.0f, -2.0f, 0.0f},
    ///                      {-2.0f, 0.0f, 0.0f}}
    ///                 )
    ///                 .with_rotation_axis_angles({rerun::RotationAxisAngle(
    ///                     {0.0f, 0.0f, 1.0f},
    ///                     rerun::Angle::degrees(static_cast<float>(i) * 2.0f)
    ///                 )})
    ///         );
    ///     }
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

        /// A color multiplier applied to the whole mesh.
        std::optional<rerun::components::AlbedoFactor> albedo_factor;

        /// Optional albedo texture.
        ///
        /// Used with the `components::Texcoord2D` of the mesh.
        ///
        /// Currently supports only sRGB(A) textures, ignoring alpha.
        /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        std::optional<rerun::components::TensorData> albedo_texture;

        /// Optional class Ids for the vertices.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
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

        /// A color multiplier applied to the whole mesh.
        Mesh3D with_albedo_factor(rerun::components::AlbedoFactor _albedo_factor) && {
            albedo_factor = std::move(_albedo_factor);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional albedo texture.
        ///
        /// Used with the `components::Texcoord2D` of the mesh.
        ///
        /// Currently supports only sRGB(A) textures, ignoring alpha.
        /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        Mesh3D with_albedo_texture(rerun::components::TensorData _albedo_texture) && {
            albedo_texture = std::move(_albedo_texture);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional class Ids for the vertices.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
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
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Mesh3D& archetype);
    };
} // namespace rerun
