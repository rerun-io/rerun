// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <memory>
#include <utility>
#include <vector>

#include "../datatypes/affix_fuzzer3.hpp"

namespace arrow {
    class DataType;
}

namespace rr {
    namespace components {
        struct AffixFuzzer16 {
            std::vector<rr::datatypes::AffixFuzzer3> many_required_unions;

          public:
            AffixFuzzer16(std::vector<rr::datatypes::AffixFuzzer3> many_required_unions)
                : many_required_unions(std::move(many_required_unions)) {}

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace components
} // namespace rr
