// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/transform_mat3x3.fbs".

#pragma once

#include "../datatypes/mat3x3.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A 3x3 transformation matrix Matrix that doesn't propagate in the transform hierarchy.
    ///
    /// 3x3 matrixes are able to represent any affine transformation in 3D space,
    /// i.e. rotation, scaling, shearing, reflection etc.
    ///
    /// Matrices in Rerun are stored as flat list of coefficients in column-major order:
    /// ```text
    ///             column 0       column 1       column 2
    ///        -------------------------------------------------
    /// row 0 | flat_columns[0] flat_columns[3] flat_columns[6]
    /// row 1 | flat_columns[1] flat_columns[4] flat_columns[7]
    /// row 2 | flat_columns[2] flat_columns[5] flat_columns[8]
    /// ```
    struct PoseTransformMat3x3 {
        rerun::datatypes::Mat3x3 matrix;

      public:
        PoseTransformMat3x3() = default;

        PoseTransformMat3x3(rerun::datatypes::Mat3x3 matrix_) : matrix(matrix_) {}

        PoseTransformMat3x3& operator=(rerun::datatypes::Mat3x3 matrix_) {
            matrix = matrix_;
            return *this;
        }

        PoseTransformMat3x3(std::array<float, 9> flat_columns_) : matrix(flat_columns_) {}

        PoseTransformMat3x3& operator=(std::array<float, 9> flat_columns_) {
            matrix = flat_columns_;
            return *this;
        }

        /// Cast to the underlying Mat3x3 datatype
        operator rerun::datatypes::Mat3x3() const {
            return matrix;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Mat3x3) == sizeof(components::PoseTransformMat3x3));

    /// \private
    template <>
    struct Loggable<components::PoseTransformMat3x3> {
        static constexpr const char Name[] = "rerun.components.PoseTransformMat3x3";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Mat3x3>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::PoseTransformMat3x3` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::PoseTransformMat3x3* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Mat3x3>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Mat3x3>::to_arrow(
                    &instances->matrix,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
