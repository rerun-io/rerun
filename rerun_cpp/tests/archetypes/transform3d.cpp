#include "archetype_test.hpp"

#include <catch2/generators/catch_generators.hpp>

#include <rerun/archetypes/transform3d.hpp>

namespace rrd = rerun::datatypes;
using namespace rerun::archetypes;

#define TEST_TAG "[transform3d][archetypes]"

SCENARIO(
    "The various utilities of Transform3D archetype produce the same data as manually constructed "
    "instances",
    TEST_TAG
) {
    const bool from_parent = GENERATE(true, false);

    SECTION("TranslationAndMat3x3") {
// Do NOT write this as rrd::Mat3x3 as this actually caught an overload resolution bug.
#define MATRIX_ILIST                              \
    {                                             \
        {1.0f, 2.0f, 3.0f}, {4.0f, 5.0f, 6.0f}, { \
            7.0f, 8.0f, 9.0f                      \
        }                                         \
    }

        rrd::Vec3D columns[3] = MATRIX_ILIST;

        Transform3D manual;
        rrd::TranslationAndMat3x3 translation_and_mat3;

        GIVEN("Transform3D from translation & matrix and from_parent==" << from_parent) {
            translation_and_mat3.translation = {1.0f, 2.0f, 3.0f};
            translation_and_mat3.from_parent = from_parent;
            manual.transform.repr = rrd::Transform3D::translation_and_mat3x3(translation_and_mat3);

            AND_GIVEN("matrix as initializer list") {
                auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, MATRIX_ILIST, true)
                                           : Transform3D({1.0f, 2.0f, 3.0f}, MATRIX_ILIST);
                translation_and_mat3.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);
                manual.transform.repr =
                    rrd::Transform3D::translation_and_mat3x3(translation_and_mat3);

                test_compare_archetype_serialization(manual, utility);
            }
            AND_GIVEN("matrix as column vectors") {
                auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, columns, true)
                                           : Transform3D({1.0f, 2.0f, 3.0f}, columns);
                translation_and_mat3.mat3x3 = columns;
                manual.transform.repr =
                    rrd::Transform3D::translation_and_mat3x3(translation_and_mat3);

                test_compare_archetype_serialization(manual, utility);
            }
        }
        GIVEN("Transform3D from matrix as initializer list and from_parent==" << from_parent) {
            translation_and_mat3.translation = std::nullopt;
            translation_and_mat3.from_parent = from_parent;

            AND_GIVEN("matrix as initializer list") {
                auto utility =
                    from_parent ? Transform3D(MATRIX_ILIST, true) : Transform3D(MATRIX_ILIST);
                translation_and_mat3.mat3x3 = rrd::Mat3x3(MATRIX_ILIST);
                manual.transform.repr =
                    rrd::Transform3D::translation_and_mat3x3(translation_and_mat3);

                test_compare_archetype_serialization(manual, utility);
            }
            AND_GIVEN("matrix as column vectors") {
                auto utility = from_parent ? Transform3D(columns, true) : Transform3D(columns);
                translation_and_mat3.mat3x3 = columns;
                manual.transform.repr =
                    rrd::Transform3D::translation_and_mat3x3(translation_and_mat3);

                test_compare_archetype_serialization(manual, utility);
            }
        }
    }

    SECTION("TranslationRotationScale") {
        const auto rotation = rrd::Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);

        Transform3D manual;
        rrd::TranslationRotationScale3D translation_rotation_scale;

        GIVEN("Transform3D from translation only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f});

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = std::nullopt;
            translation_rotation_scale.scale = std::nullopt;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from translation/rotation/scale and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, rotation, 1.0f, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, rotation, 1.0f);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from translation/scale and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, 1.0f, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, 1.0f);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = std::nullopt;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from translation/rotation and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, rotation, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, rotation);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = std::nullopt;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from rotation/scale and from_parent==" << from_parent) {
            auto utility =
                from_parent ? Transform3D(rotation, 1.0f, true) : Transform3D(rotation, 1.0f);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from rotation only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D(rotation, true) : Transform3D(rotation);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = std::nullopt;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
        GIVEN("Transform3D from scale only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D(1.0f, true) : Transform3D(1.0f);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = std::nullopt;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rrd::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_compare_archetype_serialization(manual, utility);
        }
    }
}
