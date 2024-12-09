// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/uvec2d.fbs".

#pragma once

#include "../component_descriptor.hpp"
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
    /// **Datatype**: A uint32 vector in 2D space.
    struct UVec2D {
        std::array<uint32_t, 2> xy;

      public: // START of extensions from uvec2d_ext.cpp:
        /// Construct UVec2D from x/y values.
        UVec2D(uint32_t x, uint32_t y) : xy{x, y} {}

        /// Construct UVec2D from x/y uint32_t pointer.
        explicit UVec2D(const uint32_t* xy_) : xy{xy_[0], xy_[1]} {}

        uint32_t x() const {
            return xy[0];
        }

        uint32_t y() const {
            return xy[1];
        }

        // END of extensions from uvec2d_ext.cpp, start of generated code:

      public:
        UVec2D() = default;

        UVec2D(std::array<uint32_t, 2> xy_) : xy(xy_) {}

        UVec2D& operator=(std::array<uint32_t, 2> xy_) {
            xy = xy_;
            return *this;
        }
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::UVec2D> {
        static constexpr ComponentDescriptor Descriptor = "rerun.datatypes.UVec2D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::UVec2D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::UVec2D* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const datatypes::UVec2D* elements,
            size_t num_elements
        );
    };
} // namespace rerun
