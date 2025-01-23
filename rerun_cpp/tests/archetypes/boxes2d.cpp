#include "archetype_test.hpp"

#include <rerun/archetypes/boxes2d.hpp>

using namespace rerun;
using namespace rerun::archetypes;

#define TEST_TAG "[boxes2d][archetypes]"

SCENARIO(
    "Boxes2D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder via from_half_sizes and manually") {
        auto from_builder = Boxes2D::from_half_sizes({{10.f, 9.f}, {5.f, -5.f}})
                                .with_centers({{0.f, 0.f}, {-1.f, 1.f}})
                                .with_colors({0xAA0000CC, 0x00BB00DD})
                                .with_labels({"hello", "friend"})
                                .with_radii({0.1f, 1.0f})
                                .with_draw_order(300.0f)
                                .with_class_ids({126, 127});

        Boxes2D from_manual;
        from_manual.half_sizes = ComponentBatch::from_loggable<components::HalfSize2D>(
                                     {{10.f, 9.f}, {5.f, -5.f}},
                                     Boxes2D::Descriptor_half_sizes
        )
                                     .value_or_throw();
        from_manual.centers = ComponentBatch::from_loggable<components::Position2D>(
                                  {{0.f, 0.f}, {-1.f, 1.f}},
                                  Boxes2D::Descriptor_centers
        )
                                  .value_or_throw();
        from_manual.colors = ComponentBatch::from_loggable<components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 Boxes2D::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.labels = ComponentBatch::from_loggable<components::Text>(
                                 {"hello", "friend"},
                                 Boxes2D::Descriptor_labels
        )
                                 .value_or_throw();
        from_manual.radii = ComponentBatch::from_loggable<components::Radius>(
                                {0.1f, 1.0f},
                                Boxes2D::Descriptor_radii
        )
                                .value_or_throw();
        from_manual.draw_order = ComponentBatch::from_loggable(
                                     components::DrawOrder(300.0f),
                                     Boxes2D::Descriptor_draw_order
        )
                                     .value_or_throw();
        from_manual.class_ids = ComponentBatch::from_loggable<components::ClassId>(
                                    {126, 127},
                                    Boxes2D::Descriptor_class_ids
        )
                                    .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_centers_and_half_sizes and manually") {
        auto from_builder = Boxes2D::from_centers_and_half_sizes({{1.f, 2.f}}, {{4.f, 6.f}});

        Boxes2D from_manual;
        from_manual.centers = ComponentBatch::from_loggable(
                                  components::Position2D(1.f, 2.f),
                                  Boxes2D::Descriptor_centers
        )
                                  .value_or_throw();
        from_manual.half_sizes = ComponentBatch::from_loggable(
                                     components::HalfSize2D(4.f, 6.f),
                                     Boxes2D::Descriptor_half_sizes
        )
                                     .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_sizes and manually") {
        auto from_builder = Boxes2D::from_sizes({{1.f, 2.f}});

        Boxes2D from_manual;
        from_manual.half_sizes = ComponentBatch::from_loggable(
                                     components::HalfSize2D(0.5f, 1.f),
                                     Boxes2D::Descriptor_half_sizes
        )
                                     .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_centers_and_sizes and manually") {
        auto from_builder = Boxes2D::from_centers_and_sizes({{1.f, 2.f}}, {{4.f, 6.f}});

        Boxes2D from_manual;
        from_manual.centers = ComponentBatch::from_loggable(
                                  components::Position2D(1.f, 2.f),
                                  Boxes2D::Descriptor_centers
        )
                                  .value_or_throw();
        from_manual.half_sizes = ComponentBatch::from_loggable(
                                     components::HalfSize2D(2.f, 3.f),
                                     Boxes2D::Descriptor_half_sizes
        )
                                     .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_mins_and_sizes and manually") {
        auto from_builder = Boxes2D::from_mins_and_sizes({{-1.f, -1.f}}, {{2.f, 4.f}});

        Boxes2D from_manual;
        from_manual.centers = ComponentBatch::from_loggable(
                                  components::Position2D(0.f, 1.f),
                                  Boxes2D::Descriptor_centers
        )
                                  .value_or_throw();
        from_manual.half_sizes = ComponentBatch::from_loggable(
                                     components::HalfSize2D(1.f, 2.f),
                                     Boxes2D::Descriptor_half_sizes
        )
                                     .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
