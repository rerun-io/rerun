#include "archetype_test.hpp"

#include <rerun/archetypes/points2d.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[points2d][archetypes]"

SCENARIO(
    "Points2D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = Points2D({{1.0, 2.0}, {10.0, 20.0}})
                                .with_radii({1.0, 10.0})
                                .with_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                                .with_labels({"hello", "friend"})
                                .with_class_ids({126, 127})
                                .with_keypoint_ids({1, 2})

                                    Points2D from_manual;
        from_manual.positions = {{1.0, 2.0}, {10.0, 20.0}};
        from_manual.radii = {1.0, 10.0};
        from_manual.colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.labels = {"hello", "friend"};
        from_manual.keypoint_ids = {1, 2};
        from_manual.class_ids = {126, 127};
        from_manual.instance_keys = {123ull, 124ull};

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
