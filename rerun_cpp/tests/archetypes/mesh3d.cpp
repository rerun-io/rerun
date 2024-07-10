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
        from_manual.vertex_positions = {{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}};
        from_manual.vertex_normals = {{4.0, 5.0, 6.0}, {40.0, 50.0, 60.0}};
        from_manual.vertex_colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.triangle_indices = {{1, 2, 3}, {4, 5, 6}};
        from_manual.albedo_factor = {{0xEE, 0x11, 0x22, 0x33}};
        from_manual.class_ids = {126, 127};

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
