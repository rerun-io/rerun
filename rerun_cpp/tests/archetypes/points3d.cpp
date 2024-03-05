#include "archetype_test.hpp"

#include <rerun/archetypes/points3d.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[points3d][archetypes]"

SCENARIO(
    "Points3D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = Points3D({{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}})
                                .with_radii({1.0, 10.0})
                                .with_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                                .with_labels({"hello", "friend"})
                                .with_class_ids({126, 127})
                                .with_keypoint_ids({1, 2});

        Points3D from_manual;
        from_manual.positions = {{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}};
        from_manual.radii = {1.0, 10.0};
        from_manual.colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.labels = {"hello", "friend"};
        from_manual.keypoint_ids = {1, 2};
        from_manual.class_ids = {126, 127};

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
