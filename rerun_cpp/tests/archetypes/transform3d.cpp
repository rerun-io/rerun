#include "archetype_test.hpp"

#include <catch2/generators/catch_generators.hpp>

#include <rerun/archetypes/transform3d.hpp>

namespace rrd = rerun::datatypes;
using namespace rerun::archetypes;

#define TEST_TAG "[transform3d][archetypes]"

// Something about the setup of `Transform3D manual` sets gcc off. Can't see any issue with it.
// This warning known to be notoriously unreliable, so let's ignore it here.
RERUN_DISABLE_MAYBE_UNINITIALIZED_PUSH

SCENARIO(
    "The various utilities of Transform3D archetype produce the same data as manually constructed "
    "instances",
    TEST_TAG
) {
    const bool from_parent = GENERATE(true, false);

// Do NOT write this as rrd::Mat3x3 as this actually caught an overload resolution bug.
#define MATRIX_ILIST                              \
    {                                             \
        {1.0f, 2.0f, 3.0f}, {4.0f, 5.0f, 6.0f}, { \
            7.0f, 8.0f, 9.0f                      \
        }                                         \
    }
    rrd::Vec3D columns[3] = MATRIX_ILIST;
    const auto rotation = rrd::Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);

    Transform3D manual;
    // List out everything so that GCC doesn't get nervous around uninitialized values.
    rrd::TranslationRotationScale3D manual_translation_rotation_scale;
    manual_translation_rotation_scale.translation = std::nullopt;
    manual_translation_rotation_scale.rotation = std::nullopt;
    manual.scale = std::nullopt;
    manual_translation_rotation_scale.from_parent = from_parent;
    manual.transform =
        rrd::Transform3D::translation_rotation_scale(manual_translation_rotation_scale);
    manual.mat3x3 = std::nullopt;
    manual.translation = std::nullopt;
    manual.axis_length = std::nullopt;

    GIVEN("Transform3D from translation from_parent==" << from_parent) {
        auto utility =
            Transform3D::from_translation({1.0f, 2.0f, 3.0f}).with_from_parent(from_parent);

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from 3x3 matrix and from_parent==" << from_parent) {
        manual.translation = std::nullopt;

        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_mat3x3(MATRIX_ILIST).with_from_parent(from_parent);
            manual.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_mat3x3(columns).with_from_parent(from_parent);
            manual.mat3x3 = rrd::Mat3x3(columns);

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from scale and from_parent==" << from_parent) {
        auto utility = Transform3D::from_scale({3.0f, 2.0f, 1.0f}).with_from_parent(from_parent);

        manual.scale = {3.0f, 2.0f, 1.0f};
        manual.transform.repr =
            rrd::Transform3D::translation_rotation_scale(manual_translation_rotation_scale);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & 3x3 matrix and from_parent==" << from_parent) {
        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);

        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, MATRIX_ILIST)
                               .with_from_parent(from_parent);
            manual.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, columns)
                               .with_from_parent(from_parent);
            manual.mat3x3 = rrd::Mat3x3(columns);

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from translation & scale and from_parent==" << from_parent) {
        auto utility = Transform3D::from_translation_scale({1.0f, 2.0f, 3.0f}, {3.0f, 2.0f, 1.0f})
                           .with_from_parent(from_parent);

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);
        manual.scale = {3.0f, 2.0f, 1.0f};
        manual.transform.repr =
            rrd::Transform3D::translation_rotation_scale(manual_translation_rotation_scale);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & rotation & scale and from_parent==" << from_parent) {
        auto utility = Transform3D::from_translation_rotation_scale(
                           {1.0f, 2.0f, 3.0f},
                           rotation,
                           {3.0f, 2.0f, 1.0f}
        )
                           .with_from_parent(from_parent);

        manual.translation = rerun::components::Translation3D(1.0f, 2.0f, 3.0f);
        manual_translation_rotation_scale.rotation = rotation;
        manual.scale = {3.0f, 2.0f, 1.0f};
        manual.transform.repr =
            rrd::Transform3D::translation_rotation_scale(manual_translation_rotation_scale);

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation & scale and from_parent==" << from_parent) {
        auto utility = Transform3D::from_rotation_scale(rotation, {3.0f, 2.0f, 1.0f})
                           .with_from_parent(from_parent);

        manual_translation_rotation_scale.rotation = rotation;
        manual.scale = {3.0f, 2.0f, 1.0f};
        manual.transform.repr =
            rrd::Transform3D::translation_rotation_scale(manual_translation_rotation_scale);

        test_compare_archetype_serialization(manual, utility);
    }
}

RR_DISABLE_MAYBE_UNINITIALIZED_POP
