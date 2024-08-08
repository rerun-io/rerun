#include "archetype_test.hpp"

#include <catch2/generators/catch_generators.hpp>

#include <rerun/archetypes/transform3d.hpp>

namespace rrd = rerun::datatypes;
using namespace rerun::archetypes;

#define TEST_TAG "[transform3d][archetypes]"

// Something about the setup of `Transform3D manual` sets gcc off. Can't see any issue with it.
// This warning known to be notoriously unreliable, so let's ignore it here.
RR_DISABLE_MAYBE_UNINITIALIZED_PUSH

SCENARIO(
    "The various utilities of Transform3D archetype produce the same data as manually constructed "
    "instances",
    TEST_TAG
) {
// Do NOT write this as rrd::Mat3x3 as this actually caught an overload resolution bug.
#define MATRIX_ILIST                              \
    {                                             \
        {1.0f, 2.0f, 3.0f}, {4.0f, 5.0f, 6.0f}, { \
            7.0f, 8.0f, 9.0f                      \
        }                                         \
    }
    rrd::Vec3D columns[3] = MATRIX_ILIST;
    const auto quaternion = rrd::Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);
    const auto axis_angle = rrd::RotationAxisAngle({1.0f, 2.0f, 3.0f}, rrd::Angle::degrees(90.0f));

    // List out everything so that GCC doesn't get nervous around uninitialized values.
    Transform3D manual;
    manual.scale = std::nullopt;
    manual.mat3x3 = std::nullopt;
    manual.translation = std::nullopt;
    manual.relation = std::nullopt;
    manual.axis_length = std::nullopt;

    GIVEN("Transform3D from translation") {
        auto utility = Transform3D::from_translation({1.0f, 2.0f, 3.0f});

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from 3x3 matrix") {
        manual.translation = std::nullopt;

        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_mat3x3(MATRIX_ILIST);
            manual.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_mat3x3(columns);
            manual.mat3x3 = rrd::Mat3x3(columns);

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from scale") {
        auto utility = Transform3D::from_scale({3.0f, 2.0f, 1.0f});

        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & 3x3 matrix") {
        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);

        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, MATRIX_ILIST);
            manual.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, columns);
            manual.mat3x3 = rrd::Mat3x3(columns);

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from translation & scale") {
        auto utility = Transform3D::from_translation_scale({1.0f, 2.0f, 3.0f}, {3.0f, 2.0f, 1.0f});

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);
        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & rotation (quaternion) & scale") {
        auto utility = Transform3D::from_translation_rotation_scale(
            {1.0f, 2.0f, 3.0f},
            quaternion,
            {3.0f, 2.0f, 1.0f}
        );

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);
        manual.quaternion = quaternion;
        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & rotation (axis angle) & scale") {
        auto utility = Transform3D::from_translation_rotation_scale(
            {1.0f, 2.0f, 3.0f},
            axis_angle,
            {3.0f, 2.0f, 1.0f}
        );

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);
        manual.rotation_axis_angle = axis_angle;
        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (quaternion) & scale") {
        auto utility = Transform3D::from_rotation_scale(quaternion, {3.0f, 2.0f, 1.0f});

        manual.quaternion = quaternion;
        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (axis angle) & scale") {
        auto utility = Transform3D::from_rotation_scale(axis_angle, {3.0f, 2.0f, 1.0f});

        manual.rotation_axis_angle = axis_angle;
        manual.scale = rerun::components::Scale3D(3.0f, 2.0f, 1.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (quaternion)") {
        auto utility = Transform3D::from_rotation(quaternion);
        manual.quaternion = quaternion;
        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (axis angle)") {
        auto utility = Transform3D::from_rotation(axis_angle);
        manual.rotation_axis_angle = axis_angle;
        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("A custom relation") {
        auto utility =
            Transform3D().with_relation(rerun::components::TransformRelation::ChildFromParent);
        manual.relation = rerun::components::TransformRelation::ChildFromParent;

        test_compare_archetype_serialization(manual, utility);
    }
}

RR_DISABLE_MAYBE_UNINITIALIZED_POP
