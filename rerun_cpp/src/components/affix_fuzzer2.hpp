// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../datatypes/affix_fuzzer1.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
}

namespace rr {
    namespace components {
        struct AffixFuzzer2 {
            rr::datatypes::AffixFuzzer1 single_required;

          public:
            AffixFuzzer2(rr::datatypes::AffixFuzzer1 single_required)
                : single_required(std::move(single_required)) {}

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace components
} // namespace rr
