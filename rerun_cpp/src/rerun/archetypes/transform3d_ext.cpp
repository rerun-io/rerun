#include "transform3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../compiler_utils.hpp"
#include "../rerun_sdk_export.hpp"
#include "../rotation3d.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::archetypes {
#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Identity transformation.
    ///
    /// Applying this transform does not alter an entity's transformation.
    RERUN_SDK_EXPORT static const Transform3D IDENTITY;

    /// Invalid transformation.
    ///
    /// Applying this transform will cause this entity and the entire subtree not to be visualized.
    RERUN_SDK_EXPORT static const Transform3D INVALID;

    /// Creates a new 3D transform from translation and matrix provided as 3 columns.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \çopydoc Transform3D::translation
    /// \param columns Column vectors of 3x3 matrix.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    ///
    /// _Implementation note:_ This overload is necessary, otherwise the array may be
    /// interpreted as bool and call the wrong overload.
    Transform3D(
        const components::Translation3D& translation_, const datatypes::Vec3D (&columns)[3],
        bool from_parent = false
    )
        : Transform3D(translation_, components::TransformMat3x3(columns), from_parent) {}

    /// Creates a new 3D transform from translation/matrix.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \çopydoc Transform3D::translation
    /// \param mat3x3_ \copydoc Transform3D::mat3x3
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(
        const components::Translation3D& translation_, const components::TransformMat3x3& mat3x3_,
        bool from_parent = false
    ) {
        *this = Transform3D().with_translation(translation_).with_mat3x3(mat3x3_);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From a translation applied after a 3x3 matrix.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \çopydoc Transform3D::translation
    /// \param mat3x3 \copydoc Transform3D::mat3x3
    static Transform3D from_translation_mat3x3(
        const components::Translation3D& translation, const components::TransformMat3x3& mat3x3
    ) {
        return Transform3D(translation, mat3x3, false);
    }

    /// From a translation applied after a 3x3 matrix provided as 3 columns.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \çopydoc Transform3D::translation
    /// \param columns Column vectors of 3x3 matrix.
    static Transform3D from_translation_mat3x3(
        const components::Translation3D& translation, const datatypes::Vec3D (&columns)[3]
    ) {
        return Transform3D::from_translation_mat3x3(
            translation,
            components::TransformMat3x3(columns)
        );
    }

    /// From translation only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \çopydoc Transform3D::translation
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(const components::Translation3D& translation_, bool from_parent = false) {
        *this = Transform3D().with_translation(translation_);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From a translation.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \çopydoc Transform3D::translation
    static Transform3D from_translation(const components::Translation3D& translation) {
        return Transform3D(translation, false);
    }

    /// From 3x3 matrix only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param mat3x3_ \copydoc Transform3D::mat3x3
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(const components::TransformMat3x3& mat3x3_, bool from_parent = false) {
        *this = Transform3D().with_mat3x3(mat3x3_);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From 3x3 matrix only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param mat3x3 \copydoc Transform3D::mat3x3
    static Transform3D from_mat3x3(const components::TransformMat3x3& mat3x3) {
        return Transform3D(mat3x3, false);
    }

    /// From 3x3 matrix provided as 3 columns only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param columns Column vectors of 3x3 matrix.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(const datatypes::Vec3D (&columns)[3], bool from_parent = false)
        : Transform3D(components::TransformMat3x3(columns), from_parent) {}

    /// From 3x3 matrix provided as 3 columns only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param columns Column vectors of 3x3 matrix.
    static Transform3D from_mat3x3(const datatypes::Vec3D (&columns)[3]) {
        return Transform3D(components::TransformMat3x3(columns), false);
    }

    /// Creates a new 3D transform from translation/rotation/scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param scale_ \copydoc Transform3D::scale
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(
        const components::Translation3D& translation_, const Rotation3D& rotation,
        const components::Scale3D& scale_, bool from_parent = false
    ) {
        *this = Transform3D().with_translation(translation_).with_scale(scale_).with_rotation(rotation);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// Creates a new 3D transform from translation/rotation/uniform-scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    ///
    /// _Implementation note:_ This explicit overload prevents interpretation of the float as
    /// bool, leading to a call to the wrong overload.
    Transform3D(
        const components::Translation3D& translation_, const Rotation3D& rotation,
        float uniform_scale, bool from_parent = false
    )
        : Transform3D(translation_, rotation, components::Scale3D(uniform_scale), from_parent) {}

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param scale \copydoc Transform3D::scale
    static Transform3D from_translation_rotation_scale(
        const components::Translation3D& translation, const Rotation3D& rotation,
        const components::Scale3D& scale
    ) {
        return Transform3D(translation, rotation, scale, false);
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    static Transform3D from_translation_rotation_scale(
        const components::Translation3D& translation, const Rotation3D& rotation,
        float uniform_scale
    ) {
        return Transform3D(translation, rotation, components::Scale3D(uniform_scale), false);
    }

    /// Creates a new rigid transform (translation & rotation only).
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(
        const components::Translation3D& translation_, const Rotation3D& rotation,
        bool from_parent = false
    ) {
        *this = Transform3D().with_translation(translation_).with_rotation(rotation);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From a rotation & scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \copydoc Transform3D::translation
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    static Transform3D from_translation_rotation(
        const components::Translation3D& translation, const Rotation3D& rotation
    ) {
        return Transform3D(translation, rotation, false);
    }

    /// From translation & scale only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \copydoc Transform3D::translation
    /// \param scale_ Transform3D::scale
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(
        const components::Translation3D& translation_, const components::Scale3D& scale_,
        bool from_parent = false
    ) {
        *this = Transform3D().with_translation(translation_).with_scale(scale_);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From a translation applied after a scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation \copydoc Transform3D::translation
    /// \param scale Transform3D::scale
    static Transform3D from_translation_scale(
        const components::Translation3D& translation, const components::Scale3D& scale
    ) {
        return Transform3D(translation, scale, false);
    }

    /// From translation & uniform scale only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param translation_ \copydoc Transform3D::translation
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    ///
    /// _Implementation note:_ This explicit overload prevents interpretation of the float as
    /// bool, leading to a call to the wrong overload.
    Transform3D(
        const components::Translation3D& translation_, float uniform_scale, bool from_parent = false
    )
        : Transform3D(translation_, components::Scale3D(uniform_scale), from_parent) {}

    /// From rotation & scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param scale_ Transform3D::scale
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(
        const Rotation3D& rotation, const components::Scale3D& scale_, bool from_parent = false
    ) {
        *this = Transform3D().with_scale(scale_).with_rotation(rotation);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From rotation & uniform scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    ///
    /// _Implementation note:_ This explicit overload prevents interpretation of the float as
    /// bool, leading to a call to the wrong overload.
    Transform3D(const Rotation3D& rotation, float uniform_scale, bool from_parent = false)
        : Transform3D(rotation, components::Scale3D(uniform_scale), from_parent) {}

    /// From a rotation & scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param scale Transform3D::scale
    static Transform3D from_rotation_scale(
        const Rotation3D& rotation, const components::Scale3D& scale
    ) {
        return Transform3D(rotation, scale, false);
    }

    /// From a rotation & uniform scale.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    static Transform3D from_rotation_scale(const Rotation3D& rotation, float uniform_scale) {
        return Transform3D(rotation, components::Scale3D(uniform_scale), false);
    }

    /// From rotation only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    /// \param from_parent If true, the transform relation to `TransformRelation::ChildFromParent`.
    Transform3D(const Rotation3D& rotation, bool from_parent = false) {
        *this = Transform3D().with_rotation(rotation);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From rotation only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param rotation Rotation represented either as a quaternion or axis + angle rotation.
    static Transform3D from_rotation(const Rotation3D& rotation) {
        return Transform3D(rotation, false);
    }

    /// From scale only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param scale_ If true, the transform relation to `TransformRelation::ChildFromParent`.
    /// \param from_parent \copydoc Transform3D::scale
    Transform3D(const components::Scale3D& scale_, bool from_parent = false) {
        *this = Transform3D().with_scale(scale_);
        if (from_parent) {
            *this = std::move(*this).with_relation(components::TransformRelation::ChildFromParent);
        }
    }

    /// From scale only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param scale Transform3D::scale
    static Transform3D from_scale(const components::Scale3D& scale) {
        return Transform3D(scale, false);
    }

    /// From scale only.
    /// Clears out all other components like `Transform3D::clear_fields`.
    ///
    /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
    static Transform3D from_scale(float uniform_scale) {
        return Transform3D(components::Scale3D(uniform_scale), false);
    }

    /// Set the rotation component of the transform using the `rerun::Rotation3D` utility.
    Transform3D with_rotation(const Rotation3D& rotation) && {
        if (rotation.axis_angle.has_value()) {
            *this = std::move(*this).with_rotation_axis_angle(rotation.axis_angle.value());
        }
        if (rotation.quaternion.has_value()) {
            *this = std::move(*this).with_quaternion(rotation.quaternion.value());
        }
        return std::move(*this);
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif

    /// Identity transformation.
    ///
    /// Applying this transform does not alter an entity's transformation.
    const Transform3D Transform3D::IDENTITY = Transform3D(rerun::datatypes::Mat3x3::IDENTITY);

    /// Invalid transformation.
    ///
    /// Applying this transform will cause this entity and the entire subtree not to be visualized.
    const Transform3D Transform3D::INVALID = Transform3D(rerun::datatypes::Mat3x3::INVALID);

} // namespace rerun::archetypes
