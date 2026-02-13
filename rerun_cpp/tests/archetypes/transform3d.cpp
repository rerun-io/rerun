#include "archetype_test.hpp"

#include <catch2/generators/catch_generators.hpp>

#include <rerun/archetypes/transform3d.hpp>

namespace rrd = rerun::datatypes;
namespace rrc = rerun::components;
using namespace rerun::archetypes;
using ComponentBatch = rerun::ComponentBatch;

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

    Transform3D manual = Transform3D();

    GIVEN("Transform3D from translation") {
        auto utility = Transform3D::from_translation({1.0f, 2.0f, 3.0f});

        manual.translation = ComponentBatch::from_loggable<rrc::Translation3D>(
                                 {1.0f, 2.0f, 3.0f},
                                 Transform3D::Descriptor_translation
        )
                                 .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from 3x3 matrix") {
        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_mat3x3(MATRIX_ILIST);
            manual.mat3x3 = ComponentBatch::from_loggable(
                                rrc::TransformMat3x3(MATRIX_ILIST),
                                Transform3D::Descriptor_mat3x3
            )
                                .value_or_throw();

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_mat3x3(columns);
            manual.mat3x3 = ComponentBatch::from_loggable(
                                rrc::TransformMat3x3(columns),
                                Transform3D::Descriptor_mat3x3
            )
                                .value_or_throw();

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from scale") {
        auto utility = Transform3D::from_scale({3.0f, 2.0f, 1.0f});

        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & 3x3 matrix") {
        manual.translation = ComponentBatch::from_loggable(
                                 rrc::Translation3D(1.0f, 2.0f, 3.0f),
                                 Transform3D::Descriptor_translation
        )
                                 .value_or_throw();

        AND_GIVEN("matrix as initializer list") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, MATRIX_ILIST);
            manual.mat3x3 = ComponentBatch::from_loggable(
                                rrc::TransformMat3x3(MATRIX_ILIST),
                                Transform3D::Descriptor_mat3x3
            )
                                .value_or_throw();

            test_compare_archetype_serialization(manual, utility);
        }
        AND_GIVEN("matrix as column vectors") {
            auto utility = Transform3D::from_translation_mat3x3({1.0f, 2.0f, 3.0f}, columns);
            manual.mat3x3 = ComponentBatch::from_loggable(
                                rrc::TransformMat3x3(columns),
                                Transform3D::Descriptor_mat3x3
            )
                                .value_or_throw();

            test_compare_archetype_serialization(manual, utility);
        }
    }

    GIVEN("Transform3D from translation & scale") {
        auto utility = Transform3D::from_translation_scale({1.0f, 2.0f, 3.0f}, {3.0f, 2.0f, 1.0f});

        manual.translation = ComponentBatch::from_loggable(
                                 rrc::Translation3D(1.0f, 2.0f, 3.0f),
                                 Transform3D::Descriptor_translation
        )
                                 .value_or_throw();
        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & rotation (quaternion) & scale") {
        auto utility = Transform3D::from_translation_rotation_scale(
            {1.0f, 2.0f, 3.0f},
            quaternion,
            {3.0f, 2.0f, 1.0f}
        );

        manual.translation = ComponentBatch::from_loggable(
                                 rrc::Translation3D(1.0f, 2.0f, 3.0f),
                                 Transform3D::Descriptor_translation
        )
                                 .value_or_throw();
        manual.quaternion = ComponentBatch::from_loggable(
                                rrc::RotationQuat(quaternion),
                                Transform3D::Descriptor_quaternion
        )
                                .value_or_throw();
        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from translation & rotation (axis angle) & scale") {
        auto utility = Transform3D::from_translation_rotation_scale(
            {1.0f, 2.0f, 3.0f},
            axis_angle,
            {3.0f, 2.0f, 1.0f}
        );

        manual.translation = ComponentBatch::from_loggable(
                                 rrc::Translation3D(1.0f, 2.0f, 3.0f),
                                 Transform3D::Descriptor_translation
        )
                                 .value_or_throw();
        manual.rotation_axis_angle = ComponentBatch::from_loggable(
                                         rrc::RotationAxisAngle(axis_angle),
                                         Transform3D::Descriptor_rotation_axis_angle
        )
                                         .value_or_throw();
        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (quaternion) & scale") {
        auto utility = Transform3D::from_rotation_scale(quaternion, {3.0f, 2.0f, 1.0f});

        manual.quaternion = ComponentBatch::from_loggable(
                                rrc::RotationQuat(quaternion),
                                Transform3D::Descriptor_quaternion
        )
                                .value_or_throw();
        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (axis angle) & scale") {
        auto utility = Transform3D::from_rotation_scale(axis_angle, {3.0f, 2.0f, 1.0f});

        manual.rotation_axis_angle = ComponentBatch::from_loggable(
                                         rrc::RotationAxisAngle(axis_angle),
                                         Transform3D::Descriptor_rotation_axis_angle
        )
                                         .value_or_throw();
        manual.scale = ComponentBatch::from_loggable(
                           rrc::Scale3D(3.0f, 2.0f, 1.0f),
                           Transform3D::Descriptor_scale
        )
                           .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (quaternion)") {
        auto utility = Transform3D::from_rotation(quaternion);
        manual.quaternion = ComponentBatch::from_loggable(
                                rrc::RotationQuat(quaternion),
                                Transform3D::Descriptor_quaternion
        )
                                .value_or_throw();
        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("Transform3D from rotation (axis angle)") {
        auto utility = Transform3D::from_rotation(axis_angle);
        manual.rotation_axis_angle = ComponentBatch::from_loggable(
                                         rrc::RotationAxisAngle(axis_angle),
                                         Transform3D::Descriptor_rotation_axis_angle
        )
                                         .value_or_throw();
        test_compare_archetype_serialization(manual, utility);
    }

    GIVEN("A custom relation") {
        auto utility = Transform3D().with_relation(rrc::TransformRelation::ChildFromParent);
        manual.relation = ComponentBatch::from_loggable(
                              rrc::TransformRelation::ChildFromParent,
                              Transform3D::Descriptor_relation
        )
                              .value_or_throw();

        test_compare_archetype_serialization(manual, utility);
    }
}

RR_DISABLE_MAYBE_UNINITIALIZED_POP
