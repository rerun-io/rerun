// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/tensor_dimension_selection.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: Selection of a single tensor dimension.
    struct TensorDimensionSelection {
        /// The dimension number to select.
        uint32_t dimension;

        /// Invert the direction of the dimension.
        bool invert;

      public:
        TensorDimensionSelection() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::TensorDimensionSelection> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.datatypes.TensorDimensionSelection";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::TensorDimensionSelection` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::TensorDimensionSelection* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::TensorDimensionSelection* elements,
            size_t num_elements
        );
    };
} // namespace rerun
