// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/instance_key.fbs"

#include <arrow/api.h>

#include "instance_key.hpp"

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> InstanceKey::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace components
} // namespace rr
