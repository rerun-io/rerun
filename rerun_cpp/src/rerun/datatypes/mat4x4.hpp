// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/mat4x4.fbs".

#pragma once

#include "../rerun_sdk_export.hpp"
#include "../result.hpp"
#include "vec4d.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class FixedSizeListBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A 4x4 Matrix.
    ///
    /// Matrices in Rerun are stored as flat list of coefficients in column-major order:
    /// ```text
    ///            column 0         column 1         column 2         column 3
    ///        --------------------------------------------------------------------
    /// row 0 | flat_columns[0]  flat_columns[4]  flat_columns[8]  flat_columns[12]
    /// row 1 | flat_columns[1]  flat_columns[5]  flat_columns[9]  flat_columns[13]
    /// row 2 | flat_columns[2]  flat_columns[6]  flat_columns[10] flat_columns[14]
    /// row 3 | flat_columns[3]  flat_columns[7]  flat_columns[11] flat_columns[15]
    /// ```
    struct Mat4x4 {
        /// Flat list of matrix coefficients in column-major order.
        std::array<float, 16> flat_columns;

      public:
        // Extensions to generated type defined in 'mat4x4_ext.cpp'

        RERUN_SDK_EXPORT static const Mat4x4 IDENTITY;

        /// Creates a new 4x4 matrix from 3 *columns* of 4 elements each.
        Mat4x4(const Vec4D (&columns)[4])
            : flat_columns{
                  columns[0].x(),
                  columns[0].y(),
                  columns[0].z(),
                  columns[0].w(),
                  columns[1].x(),
                  columns[1].y(),
                  columns[1].z(),
                  columns[1].w(),
                  columns[2].x(),
                  columns[2].y(),
                  columns[2].z(),
                  columns[2].w(),
                  columns[3].x(),
                  columns[3].y(),
                  columns[3].z(),
                  columns[3].w(),
              } {}

        /// Construct a new 4x4 matrix from a pointer to 16 floats (in column major order).
        explicit Mat4x4(const float* elements)
            : flat_columns{
                  elements[0],
                  elements[1],
                  elements[2],
                  elements[3],
                  elements[4],
                  elements[5],
                  elements[6],
                  elements[7],
                  elements[8],
                  elements[9],
                  elements[10],
                  elements[11],
                  elements[12],
                  elements[13],
                  elements[14],
                  elements[15],
              } {}

      public:
        Mat4x4() = default;

        Mat4x4(std::array<float, 16> flat_columns_) : flat_columns(flat_columns_) {}

        Mat4x4& operator=(std::array<float, 16> flat_columns_) {
            flat_columns = flat_columns_;
            return *this;
        }
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::Mat4x4> {
        static constexpr const char Name[] = "rerun.datatypes.Mat4x4";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::Mat4x4` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::Mat4x4* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const datatypes::Mat4x4* elements,
            size_t num_elements
        );
    };
} // namespace rerun
