// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/fill_mode.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
    class DataType;
    class UInt8Type;
    using UInt8Builder = NumericBuilder<UInt8Type>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: How a geometric shape is drawn and colored.
    enum class FillMode : uint8_t {

        /// Lines are drawn around the edges of the shape.
        ///
        /// The interior (2D) or surface (3D) are not drawn.
        Wireframe = 1,

        /// The interior (2D) or surface (3D) is filled with a single color.
        ///
        /// Lines are not drawn.
        Solid = 2,
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::FillMode> {
        static constexpr const char Name[] = "rerun.components.FillMode";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::FillMode` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::FillMode* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt8Builder* builder, const components::FillMode* elements, size_t num_elements
        );
    };
} // namespace rerun
