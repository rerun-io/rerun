// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include <arrow/api.h>

#include "affix_fuzzer2.hpp"

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> AffixFuzzer2::to_arrow_datatype() {
            return arrow::struct_({});
        }
    } // namespace datatypes
} // namespace rr
