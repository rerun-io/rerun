#include "archetype_test.hpp"

#include <rerun/archetypes/points3d.hpp>

using namespace rerun;
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
                                .with_keypoint_ids({1, 2})
                                .with_show_labels(true);

        Points3D from_manual;
        from_manual.positions = ComponentBatch::from_loggable<components::Position3D>(
                                    {{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}},
                                    Points3D::Descriptor_positions
        )
                                    .value_or_throw();
        from_manual.radii = ComponentBatch::from_loggable<components::Radius>(
                                {1.0, 10.0},
                                Points3D::Descriptor_radii
        )
                                .value_or_throw();
        from_manual.colors = ComponentBatch::from_loggable<components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 Points3D::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.labels = ComponentBatch::from_loggable<components::Text>(
                                 {"hello", "friend"},
                                 Points3D::Descriptor_labels
        )
                                 .value_or_throw();
        from_manual.show_labels = ComponentBatch::from_loggable(
                                      components::ShowLabels(true),
                                      Points3D::Descriptor_show_labels
        )
                                      .value_or_throw();
        from_manual.class_ids = ComponentBatch::from_loggable<components::ClassId>(
                                    {126, 127},
                                    Points3D::Descriptor_class_ids
        )
                                    .value_or_throw();
        from_manual.keypoint_ids = ComponentBatch::from_loggable<components::KeypointId>(
                                       {1, 2},
                                       Points3D::Descriptor_keypoint_ids
        )
                                       .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
