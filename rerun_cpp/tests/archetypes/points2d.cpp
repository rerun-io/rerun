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
                                .with_keypoint_ids({1, 2});

        Points2D from_manual;
        from_manual.positions = rerun::ComponentBatch::from_loggable<rerun::components::Position2D>(
                                    {{1.0, 2.0}, {10.0, 20.0}},
                                    Points2D::Descriptor_positions
        )
                                    .value_or_throw();
        from_manual.radii = rerun::ComponentBatch::from_loggable<rerun::components::Radius>(
                                {1.0, 10.0},
                                Points2D::Descriptor_radii
        )
                                .value_or_throw();
        from_manual.colors = rerun::ComponentBatch::from_loggable<rerun::components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 Points2D::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.labels = rerun::ComponentBatch::from_loggable<rerun::components::Text>(
                                 {"hello", "friend"},
                                 Points2D::Descriptor_labels
        )
                                 .value_or_throw();
        from_manual.keypoint_ids =
            rerun::ComponentBatch::from_loggable<rerun::components::KeypointId>(
                {1, 2},
                Points2D::Descriptor_keypoint_ids
            )
                .value_or_throw();
        from_manual.class_ids = rerun::ComponentBatch::from_loggable<rerun::components::ClassId>(
                                    {126, 127},
                                    Points2D::Descriptor_class_ids
        )
                                    .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
