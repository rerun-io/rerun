// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#pragma once

#include "../datatypes/rotation3d.hpp"
#include "../datatypes/scale3d.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>

namespace rerun {
    namespace datatypes {
        /// Representation of an affine transform via separate translation, rotation & scale.
        struct TranslationRotationScale3D {
            /// 3D translation vector, applied last.
            std::optional<rerun::datatypes::Vec3D> translation;

            /// 3D rotation, applied second.
            std::optional<rerun::datatypes::Rotation3D> rotation;

            /// 3D scale, applied first.
            std::optional<rerun::datatypes::Scale3D> scale;

            /// If true, the transform maps from the parent space to the space where the transform
            /// was logged. Otherwise, the transform maps from the space to its parent.
            bool from_parent;

          public:
            // Extensions to generated type defined in 'translation_rotation_scale3d_ext.cpp'

            static const TranslationRotationScale3D IDENTITY;

            TranslationRotationScale3D(
                const std::optional<Vec3D>& _translation,
                const std::optional<Rotation3D>& _rotation, const std::optional<Scale3D>& _scale,
                bool _from_parent
            )
                : translation(_translation),
                  rotation(_rotation),
                  scale(_scale),
                  from_parent(_from_parent) {}

            /// From translation & rotation only.
            TranslationRotationScale3D(
                const Vec3D& _translation, const Rotation3D& _rotation, bool _from_parent = false
            )
                : translation(_translation),
                  rotation(_rotation),
                  scale(std::nullopt),
                  from_parent(_from_parent) {}

            /// From translation & scale only.
            TranslationRotationScale3D(
                const Vec3D& _translation, const Scale3D& _scale, bool _from_parent = false
            )
                : translation(_translation),
                  rotation(std::nullopt),
                  scale(_scale),
                  from_parent(_from_parent) {}

            /// Creates a new rigid transform (translation & rotation only).
            static TranslationRotationScale3D rigid(
                const Vec3D& _translation, const Rotation3D& _rotation, bool _from_parent = false
            ) {
                return TranslationRotationScale3D(_translation, _rotation, _from_parent);
            }

            /// Creates a new affine transform
            static TranslationRotationScale3D affine(
                const Vec3D& _translation, const Rotation3D& _rotation, const Scale3D& _scale,
                bool _from_parent = false
            ) {
                return TranslationRotationScale3D(_translation, _rotation, _scale, _from_parent);
            }

          public:
            TranslationRotationScale3D() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const TranslationRotationScale3D* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
