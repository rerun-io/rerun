#include "archetype_test.hpp"

#include <catch2/generators/catch_generators.hpp>

#include <rerun/archetypes/transform3d.hpp>

namespace rr = rerun;
using namespace rr::archetypes;

#define TEST_TAG "[transform3d]"

SCENARIO(
    "The various utilities of Transform3D archetype produce the same data as manually constructed "
    "instances",
    TEST_TAG
) {
    const bool from_parent = GENERATE(true, false);

    SECTION("TranslationAndMat3x3") {
        // Do NOT write this as rr::datatypes::Mat3x3 as this actually caught an overload resolution
        // bug.
        const rr::datatypes::Vec3D matrix[3] = {
            {1.0f, 2.0f, 3.0f},
            {4.0f, 5.0f, 6.0f},
            {7.0f, 8.0f, 9.0f}};

        Transform3D manual;
        rr::datatypes::TranslationAndMat3x3 translation_and_mat3;

        GIVEN("Transform3D from translation & matrix and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, matrix, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, matrix);

            translation_and_mat3.translation = {1.0f, 2.0f, 3.0f};
            translation_and_mat3.matrix = matrix;
            translation_and_mat3.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from translation only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f});

            translation_and_mat3.translation = {1.0f, 2.0f, 3.0f};
            translation_and_mat3.matrix = std::nullopt;
            translation_and_mat3.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from matrix only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D(matrix, true) : Transform3D(matrix);

            translation_and_mat3.translation = std::nullopt;
            translation_and_mat3.matrix = matrix;
            translation_and_mat3.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3);

            test_serialization_for_manual_and_builder(manual, utility);
        }
    }

    SECTION("TranslationRotationScale") {
        const auto rotation = rr::datatypes::Quaternion{1.0f, 2.0f, 3.0f, 4.0f};

        Transform3D manual;
        rr::datatypes::TranslationRotationScale3D translation_rotation_scale;

        GIVEN("Transform3D from translation/rotation/scale and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, rotation, 1.0f, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, rotation, 1.0f);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from translation/scale and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, 1.0f, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, 1.0f);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = std::nullopt;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from translation/rotation and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D({1.0f, 2.0f, 3.0f}, rotation, true)
                                       : Transform3D({1.0f, 2.0f, 3.0f}, rotation);

            translation_rotation_scale.translation = {1.0f, 2.0f, 3.0f};
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = std::nullopt;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from rotation/scale and from_parent==" << from_parent) {
            auto utility =
                from_parent ? Transform3D(rotation, 1.0f, true) : Transform3D(rotation, 1.0f);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from rotation only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D(rotation, true) : Transform3D(rotation);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = rotation;
            translation_rotation_scale.scale = std::nullopt;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
        GIVEN("Transform3D from scale only and from_parent==" << from_parent) {
            auto utility = from_parent ? Transform3D(1.0f, true) : Transform3D(1.0f);

            translation_rotation_scale.translation = std::nullopt;
            translation_rotation_scale.rotation = std::nullopt;
            translation_rotation_scale.scale = 1.0f;
            translation_rotation_scale.from_parent = from_parent;
            manual.transform.repr =
                rr::datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale);

            test_serialization_for_manual_and_builder(manual, utility);
        }
    }
}
