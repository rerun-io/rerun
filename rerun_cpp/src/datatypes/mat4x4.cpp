// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/mat4x4.fbs"

#include <arrow/api.h>

#include "mat4x4.hpp"

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> Mat4x4::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace datatypes
} // namespace rr
