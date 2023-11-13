// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/viewport_layout.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <vector>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun::blueprint {
    /// **Blueprint**: A view of a space.
    ///
    /// Unstable. Used for the ongoing blueprint experimentations.
    struct ViewportLayout {
        /// space_view_keys
        std::vector<uint8_t> space_view_keys;

        /// tree
        std::vector<uint8_t> tree;

        /// auto_layout
        bool auto_layout;

      public:
        ViewportLayout() = default;

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const ViewportLayout* elements, size_t num_elements
        );
    };
} // namespace rerun::blueprint
