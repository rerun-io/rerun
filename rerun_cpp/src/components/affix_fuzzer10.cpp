// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include <arrow/api.h>

#include "affix_fuzzer10.hpp"

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer10::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace components
} // namespace rr
