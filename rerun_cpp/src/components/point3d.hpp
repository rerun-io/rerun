// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/point3d.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/point3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rr {
    namespace components {
        /// A point in 3D space.
        struct Point3D {
            rr::datatypes::Point3D xy;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            Point3D(rr::datatypes::Point3D xy) : xy(std::move(xy)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const Point3D* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Point3D components.
            static arrow::Result<rr::DataCell> to_data_cell(
                const Point3D* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rr
