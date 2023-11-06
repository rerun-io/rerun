// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/disconnected_space.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class BooleanBuilder;
    class DataType;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        /// **Component**: Specifies that the entity path at which this is logged is disconnected from its parent.
        ///
        /// This is useful for specifying that a subgraph is independent of the rest of the scene.
        ///
        /// If a transform or pinhole is logged on the same path, this component will be ignored.
        struct DisconnectedSpace {
            /// Whether the entity path at which this is logged is disconnected from its parent.
            bool is_disconnected;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            DisconnectedSpace() = default;

            DisconnectedSpace(bool is_disconnected_) : is_disconnected(is_disconnected_) {}

            DisconnectedSpace& operator=(bool is_disconnected_) {
                is_disconnected = is_disconnected_;
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::BooleanBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static rerun::Error fill_arrow_array_builder(
                arrow::BooleanBuilder* builder, const DisconnectedSpace* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of DisconnectedSpace components.
            static Result<rerun::DataCell> to_data_cell(
                const DisconnectedSpace* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
