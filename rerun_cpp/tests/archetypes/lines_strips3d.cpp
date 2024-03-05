#include "archetype_test.hpp"

#include <rerun/archetypes/line_strips3d.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[linestrips3d][archetypes]"

SCENARIO(
    "LineStrips3D archetype can be serialized with the same result for manually built instances "
    "and the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder =
            LineStrips3D({
                             rerun::components::LineStrip3D({{0.f, 0.f, 0.f}, {2.f, 1.f, -1.f}}),
                             rerun::components::LineStrip3D({{4.f, -1.f, 3.f}, {6.f, 0.f, 1.5f}}),
                         })
                .with_radii({1.0, 10.0})
                .with_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                .with_labels({"hello", "friend"})
                .with_class_ids({126, 127});

        LineStrips3D from_manual;
        from_manual.strips = {
            rerun::components::LineStrip3D({{0.f, 0.f, 0.f}, {2.f, 1.f, -1.f}}),
            rerun::components::LineStrip3D({{4.f, -1.f, 3.f}, {6.f, 0.f, 1.5f}}),
        };
        from_manual.radii = {1.0, 10.0};
        from_manual.colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.labels = {"hello", "friend"};
        from_manual.class_ids = {126, 127};

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
