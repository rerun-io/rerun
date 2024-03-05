#include "archetype_test.hpp"

#include <rerun/archetypes/boxes3d.hpp>

using namespace rerun::archetypes;
using namespace rerun::datatypes;

#define TEST_TAG "[boxes3d][archetypes]"

SCENARIO(
    "Boxes3D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder via from_half_sizes and manually") {
        auto from_builder = Boxes3D::from_half_sizes({{10.f, 9.f, 8.f}, {5.f, -5.f, 5.f}})
                                .with_centers({{0.f, 0.f, 0.f}, {-1.f, 1.f, -2.f}})
                                .with_rotations({
                                    Quaternion::from_xyzw(0.f, 1.f, 2.f, 3.f),
                                    RotationAxisAngle({0.f, 1.f, 2.f}, Angle::radians(45.f)),
                                })
                                .with_colors({0xAA0000CC, 0x00BB00DD})
                                .with_labels({"hello", "friend"})
                                .with_radii({0.1f, 1.0f})
                                .with_class_ids({126, 127});

        Boxes3D from_manual;
        from_manual.half_sizes = {{10.f, 9.f, 8.f}, {5.f, -5.f, 5.f}};
        from_manual.centers = {{0.f, 0.f, 0.f}, {-1.f, 1.f, -2.f}};
        from_manual.rotations = {
            Quaternion::from_xyzw(0.f, 1.f, 2.f, 3.f),
            RotationAxisAngle({0.f, 1.f, 2.f}, Angle::radians(45.f)),
        };
        from_manual.colors = {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}};
        from_manual.labels = {"hello", "friend"};
        from_manual.radii = {0.1f, 1.0f};
        from_manual.class_ids = {126, 127};

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_centers_and_half_sizes and manually") {
        auto from_builder =
            Boxes3D::from_centers_and_half_sizes({{1.f, 2.f, 3.f}}, {{4.f, 6.f, 8.f}});

        Boxes3D from_manual;
        from_manual.centers = {{1.f, 2.f, 3.f}};
        from_manual.half_sizes = {{4.f, 6.f, 8.f}};

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_sizes and manually") {
        auto from_builder = Boxes3D::from_sizes({{1.f, 2.f, 3.f}});

        Boxes3D from_manual;
        from_manual.half_sizes = {{0.5f, 1.f, 1.5f}};

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_centers_and_sizes and manually") {
        auto from_builder = Boxes3D::from_centers_and_sizes({{1.f, 2.f, 3.f}}, {{4.f, 6.f, 8.f}});

        Boxes3D from_manual;
        from_manual.centers = {{1.f, 2.f, 3.f}};
        from_manual.half_sizes = {{2.f, 3.f, 4.f}};

        test_compare_archetype_serialization(from_manual, from_builder);
    }

    GIVEN("Constructed from via from_mins_and_sizes and manually") {
        auto from_builder = Boxes3D::from_mins_and_sizes({{-1.f, -1.f, -1.f}}, {{2.f, 4.f, 2.f}});

        Boxes3D from_manual;
        from_manual.centers = {{0.f, 1.f, 0.f}};
        from_manual.half_sizes = {{1.f, 2.f, 1.f}};

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
