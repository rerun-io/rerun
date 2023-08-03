// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_and_mat3x3.fbs"

#pragma once

#include "../datatypes/mat3x3.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>

namespace rerun {
    namespace datatypes {
        /// Representation of an affine transform via a 3x3 affine matrix paired with a translation.
        ///
        /// First applies the matrix, then the translation.
        struct TranslationAndMat3x3 {
            /// 3D translation, applied after the matrix.
            std::optional<rerun::datatypes::Vec3D> translation;

            /// 3x3 matrix for scale, rotation & shear.
            std::optional<rerun::datatypes::Mat3x3> matrix;

            /// If true, the transform maps from the parent space to the space where the transform
            /// was logged. Otherwise, the transform maps from the space to its parent.
            bool from_parent;

          public:
            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const TranslationAndMat3x3* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
