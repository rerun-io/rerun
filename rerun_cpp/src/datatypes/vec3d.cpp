// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/vec3d.fbs"

#include <arrow/api.h>

#include "vec3d.hpp"

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> Vec3D::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace datatypes
} // namespace rr
