// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/vec4d.fbs".

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
        /// **Datatype**: A vector in 4D space.
        struct Vec4D {
            float xyzw[4];

          public:
            // Extensions to generated type defined in 'vec4d_ext.cpp'

            /// Construct Vec4D from x/y/z/w values.
            Vec4D(float x, float y, float z, float w) : xyzw{x, y, z, w} {}

            /// Construct Vec4D from x/y/z/w float pointer.
            ///
            /// Attention: The pointer must point to at least least 4 floats long.
            Vec4D(const float* ptr) : xyzw{ptr[0], ptr[1], ptr[2], ptr[3]} {}

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
            Vec4D() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Vec4D* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
