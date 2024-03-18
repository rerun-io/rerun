// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/uvec4d.fbs".

#pragma once

#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class FixedSizeListBuilder;
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
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::UVec4D> {
        static constexpr const char Name[] = "rerun.datatypes.UVec4D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::UVec4D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::UVec4D* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const datatypes::UVec4D* elements,
            size_t num_elements
        );
    };
} // namespace rerun
