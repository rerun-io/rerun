// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/uvec4d.fbs".

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

namespace rerun::datatypes {
    /// **Datatype**: A uint vector in 4D space.
    struct UVec4D {
        std::array<uint32_t, 4> xyzw;

      public:
        UVec4D() = default;

        UVec4D(std::array<uint32_t, 4> xyzw_) : xyzw(xyzw_) {}

        UVec4D& operator=(std::array<uint32_t, 4> xyzw_) {
            xyzw = xyzw_;
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const UVec4D* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
