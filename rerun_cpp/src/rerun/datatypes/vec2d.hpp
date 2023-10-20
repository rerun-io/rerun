// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/vec2d.fbs".

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
        /// **Datatype**: A vector in 2D space.
        struct Vec2D {
            std::array<float, 2> xy;

          public:
            // Extensions to generated type defined in 'vec2d_ext.cpp'

            /// Construct Vec2D from x/y values.
            Vec2D(float x, float y) : xy{x, y} {}

            /// Construct Vec2D from x/y float pointer.
            ///
            /// Attention: The pointer must point to at least least 2 floats long.
            Vec2D(const float* ptr) : xy{ptr[0], ptr[1]} {}

            float x() const {
                return xy[0];
            }

            float y() const {
                return xy[1];
            }

          public:
            Vec2D() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Vec2D* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
