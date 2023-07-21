// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/point3d.fbs"

#pragma once

#include <arrow/result.h>
#include <cstdint>
#include <memory>

namespace arrow {
    class ArrayBuilder;
    class DataType;
    class MemoryPool;
} // namespace arrow

namespace rr {
    namespace datatypes {
        /// A point in 3D space.
        struct Point3D {
            float x;

            float y;

            float z;

          public:
            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();

            /// Fills out an arrow array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> to_arrow(
                arrow::MemoryPool* memory_pool, const Point3D* elements, size_t num_elements);
        };
    } // namespace datatypes
} // namespace rr
