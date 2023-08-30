// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/image_variant.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/image_variant.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        struct ImageVariant {
            rerun::datatypes::ImageVariant variant;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            ImageVariant() = default;

            ImageVariant(rerun::datatypes::ImageVariant _variant) : variant(std::move(_variant)) {}

            ImageVariant& operator=(rerun::datatypes::ImageVariant _variant) {
                variant = std::move(_variant);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder* builder, const ImageVariant* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of ImageVariant components.
            static Result<rerun::DataCell> to_data_cell(
                const ImageVariant* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
