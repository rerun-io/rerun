// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/transform3d.fbs"

#include "transform3d.hpp"

#include "../datatypes/transform3d.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> Transform3D::to_arrow_datatype() {
            return rr::datatypes::Transform3D::to_arrow_datatype();
        }
    } // namespace components
} // namespace rr
