// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs"

#include <arrow/api.h>

#include "quaternion.hpp"

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> Quaternion::to_arrow_datatype() {
            return arrow::fixed_size_list(arrow::field("item", arrow::float32(), false, nullptr),
                                          4);
        }
    } // namespace datatypes
} // namespace rr
