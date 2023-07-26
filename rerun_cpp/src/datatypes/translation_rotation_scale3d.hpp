// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#pragma once

#include "../datatypes/rotation3d.hpp"
#include "../datatypes/scale3d.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>

namespace rr {
    namespace datatypes {
        /// Representation of an affine transform via separate translation, rotation & scale.
        struct TranslationRotationScale3D {
            /// 3D translation vector, applied last.
            std::optional<rr::datatypes::Vec3D> translation;

            /// 3D rotation, applied second.
            std::optional<rr::datatypes::Rotation3D> rotation;

            /// 3D scale, applied first.
            std::optional<rr::datatypes::Scale3D> scale;

            /// If true, the transform maps from the parent space to the space where the transform
            /// was logged. Otherwise, the transform maps from the space to its parent.
            bool from_parent;

          public:
            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();

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
} // namespace rr
