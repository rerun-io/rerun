// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/auto_space_views.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class BooleanBuilder;
    class DataType;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace blueprint {
        /// **Blueprint**: A flag indicating space views should be automatically populated.
        ///
        /// Unstable. Used for the ongoing blueprint experimentations.
        struct AutoSpaceViews {
            bool enabled;

          public:
            AutoSpaceViews() = default;

            AutoSpaceViews(bool enabled_) : enabled(enabled_) {}

            AutoSpaceViews& operator=(bool enabled_) {
                enabled = enabled_;
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::BooleanBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::BooleanBuilder* builder, const AutoSpaceViews* elements, size_t num_elements
            );
        };
    } // namespace blueprint
} // namespace rerun
