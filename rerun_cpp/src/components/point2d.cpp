// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/point2d.fbs"

#include <arrow/api.h>

#include "point2d.hpp"

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> Point2D::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace components
} // namespace rr
