// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include "affix_fuzzer5.hpp"

#include "../datatypes/affix_fuzzer4.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> AffixFuzzer5::to_arrow_datatype() {
            return arrow::struct_({
                arrow::field("single_optional_union",
                             rr::datatypes::AffixFuzzer4::to_arrow_datatype(),
                             true,
                             nullptr),
            });
        }
    } // namespace datatypes
} // namespace rr
