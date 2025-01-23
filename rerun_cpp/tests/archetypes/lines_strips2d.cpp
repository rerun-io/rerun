#include "archetype_test.hpp"

#include <rerun/archetypes/line_strips2d.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[linestrips2d][archetypes]"

SCENARIO(
    "LineStrips2D archetype can be serialized with the same result for manually built instances "
    "and the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder =
            LineStrips2D({
                             rerun::components::LineStrip2D({{0.f, 0.f}, {1.f, -1.f}}),
                             rerun::components::LineStrip2D({{-1.f, 3.f}, {0.f, 1.5f}}),
                         })
                .with_radii({1.0, 10.0})
                .with_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                .with_labels({"hello", "friend"})
                .with_class_ids({126, 127})
                .with_draw_order(123);

        LineStrips2D from_manual;
        from_manual.strips = rerun::ComponentBatch::from_loggable<rerun::components::LineStrip2D>(
                                 {
                                     rerun::components::LineStrip2D({{0.f, 0.f}, {1.f, -1.f}}),
                                     rerun::components::LineStrip2D({{-1.f, 3.f}, {0.f, 1.5f}}),
                                 },
                                 LineStrips2D::Descriptor_strips
        )
                                 .value_or_throw();
        from_manual.radii = rerun::ComponentBatch::from_loggable<rerun::components::Radius>(
                                {1.0, 10.0},
                                LineStrips2D::Descriptor_radii
        )
                                .value_or_throw();
        from_manual.colors = rerun::ComponentBatch::from_loggable<rerun::components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 LineStrips2D::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.labels = rerun::ComponentBatch::from_loggable<rerun::components::Text>(
                                 {"hello", "friend"},
                                 LineStrips2D::Descriptor_labels
        )
                                 .value_or_throw();
        from_manual.class_ids = rerun::ComponentBatch::from_loggable<rerun::components::ClassId>(
                                    {126, 127},
                                    LineStrips2D::Descriptor_class_ids
        )
                                    .value_or_throw();
        from_manual.draw_order = rerun::ComponentBatch::from_loggable(
                                     rerun::components::DrawOrder(123.0f),
                                     LineStrips2D::Descriptor_draw_order
        )
                                     .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
