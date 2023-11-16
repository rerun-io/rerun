// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/entity_path.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StringBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A path to an entity in the `DataStore`.
    struct EntityPath {
        std::string path;

      public:
        EntityPath() = default;

        EntityPath(std::string path_) : path(std::move(path_)) {}

        EntityPath& operator=(std::string path_) {
            path = std::move(path_);
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::StringBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StringBuilder* builder, const EntityPath* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
