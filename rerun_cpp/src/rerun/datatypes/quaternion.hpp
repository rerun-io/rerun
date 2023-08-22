// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs"

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// A Quaternion represented by 4 real numbers.
        struct Quaternion {
            float xyzw[4];

          public:
            // Extensions to generated type defined in 'quaternion_ext.cpp'

            /// Construct Quaternion from x/y/z/w values.
            Quaternion(float x, float y, float z, float w) : xyzw{x, y, z, w} {}

            float x() const {
                return xyzw[0];
            }

            float y() const {
                return xyzw[1];
            }

            float z() const {
                return xyzw[2];
            }

            float w() const {
                return xyzw[3];
            }

          public:
            Quaternion() = default;

            Quaternion(const float (&_xyzw)[4]) : xyzw{_xyzw[0], _xyzw[1], _xyzw[2], _xyzw[3]} {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Quaternion* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
