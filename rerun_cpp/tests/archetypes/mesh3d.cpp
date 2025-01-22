#include "archetype_test.hpp"

#include <rerun/archetypes/mesh3d.hpp>

using namespace rerun::archetypes;
using namespace rerun::components;

#define TEST_TAG "[mesh3d][archetypes]"

SCENARIO(
    "Mesh3D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder =
            Mesh3D({{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}})
                .with_vertex_normals({{4.0, 5.0, 6.0}, {40.0, 50.0, 60.0}})
                .with_vertex_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                .with_triangle_indices({{1, 2, 3}, {4, 5, 6}})
                .with_albedo_factor(0xEE112233)
                .with_class_ids({126, 127});

        Mesh3D from_manual;
        from_manual.vertex_positions =
            rerun::ComponentBatch::from_loggable<rerun::components::Position3D>(
                {{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}},
                Mesh3D::Descriptor_vertex_positions
            )
                .value_or_throw();
        from_manual.vertex_normals =
            rerun::ComponentBatch::from_loggable<rerun::components::Vector3D>(
                {{4.0, 5.0, 6.0}, {40.0, 50.0, 60.0}},
                Mesh3D::Descriptor_vertex_normals
            )
                .value_or_throw();
        from_manual.vertex_colors = rerun::ComponentBatch::from_loggable<rerun::components::Color>(
                                        {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                        Mesh3D::Descriptor_vertex_colors
        )
                                        .value_or_throw();
        from_manual.triangle_indices =
            rerun::ComponentBatch::from_loggable<rerun::components::TriangleIndices>(
                {{1, 2, 3}, {4, 5, 6}},
                Mesh3D::Descriptor_triangle_indices
            )
                .value_or_throw();
        from_manual.albedo_factor = rerun::ComponentBatch::from_loggable(
                                        rerun::components::AlbedoFactor({0xEE, 0x11, 0x22, 0x33}),
                                        Mesh3D::Descriptor_albedo_factor
        )
                                        .value_or_throw();
        from_manual.class_ids = rerun::ComponentBatch::from_loggable<rerun::components::ClassId>(
                                    {126, 127},
                                    Mesh3D::Descriptor_class_ids
        )
                                    .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
