// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/translation_and_mat3x3.fbs".

#pragma once

#include "../result.hpp"
#include "mat3x3.hpp"
#include "vec3d.hpp"

#include <cstdint>
#include <memory>
#include <optional>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// **Datatype**: Representation of an affine transform via a 3x3 affine matrix paired with a translation.
        ///
        /// First applies the matrix, then the translation.
        struct TranslationAndMat3x3 {
            /// 3D translation, applied after the matrix.
            std::optional<rerun::datatypes::Vec3D> translation;

            /// 3x3 matrix for scale, rotation & shear.
            std::optional<rerun::datatypes::Mat3x3> mat3x3;

            /// If true, this transform is from the parent space to the space where the transform was logged.
            ///
            /// If false (default), the transform maps from this space to its parent,
            /// i.e. the translation is the position in the parent space.
            bool from_parent;

          public:
            // Extensions to generated type defined in 'translation_and_mat3x3_ext.cpp'

            static const TranslationAndMat3x3 IDENTITY;

            /// Creates a new 3D transform from translation/matrix.
            ///
            /// \param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(
                const std::optional<Vec3D>& _translation, const std::optional<Mat3x3>& _mat3x3,
                bool _from_parent
            )
                : translation(_translation), mat3x3(_mat3x3), from_parent(_from_parent) {}

            /// From rotation only.
            ///
            /// \param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(const Mat3x3& _mat3x3, bool _from_parent = false)
                : translation(std::nullopt), mat3x3(_mat3x3), from_parent(_from_parent) {}

            /// From translation only.
            ///
            /// \param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(const Vec3D& _translation, bool _from_parent = false)
                : translation(_translation), mat3x3(std::nullopt), from_parent(_from_parent) {}

          public:
            TranslationAndMat3x3() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static rerun::Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const TranslationAndMat3x3* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
