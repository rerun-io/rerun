// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/annotation_context.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/class_description_map_elem.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>
#include <vector>

namespace rr {
    namespace components {
        /// The `AnnotationContext` provides additional information on how to display
        /// entities.
        ///
        /// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
        /// the labels and colors will be looked up in the appropriate
        ///`AnnotationContext`. We use the *first* annotation context we find in the
        /// path-hierarchy when searching up through the ancestors of a given entity
        /// path.
        struct AnnotationContext {
            std::vector<rr::datatypes::ClassDescriptionMapElem> class_map;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            AnnotationContext(std::vector<rr::datatypes::ClassDescriptionMapElem> class_map)
                : class_map(std::move(class_map)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::ListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::ListBuilder* builder, const AnnotationContext* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AnnotationContext components.
            static arrow::Result<rr::DataCell> to_data_cell(
                const AnnotationContext* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rr
