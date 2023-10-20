// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/vec3d.fbs".

#pragma once

#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// **Datatype**: A vector in 3D space.
        struct Vec3D {
            std::array<float, 3> xyz;

          public:
            // Extensions to generated type defined in 'vec3d_ext.cpp'

            /// Construct Vec3D from x/y/z values.
            Vec3D(float x, float y, float z) : xyz{x, y, z} {}

            /// Construct Vec3D from x/y/z float pointer.
            ///
            /// Attention: The pointer must point to at least least 3 floats long.
            Vec3D(const float* ptr) : xyz{ptr[0], ptr[1], ptr[2]} {}

            float x() const {
                return xyz[0];
            }

            float y() const {
                return xyz[1];
            }

            float z() const {
                return xyz[2];
            }

          public:
            Vec3D() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Vec3D* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
