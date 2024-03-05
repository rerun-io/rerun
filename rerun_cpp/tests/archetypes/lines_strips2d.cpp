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
        from_manual.strips = {
            rerun::components::LineStrip2D({{0.f, 0.f}, {1.f, -1.f}}),
            rerun::components::LineStrip2D({{-1.f, 3.f}, {0.f, 1.5f}}),
        };
        from_manual.radii = {1.0, 10.0};
        from_manual.colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.labels = {"hello", "friend"};
        from_manual.class_ids = {126, 127};
        from_manual.draw_order = 123.0f;

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
