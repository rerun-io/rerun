// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <memory>
#include <optional>
#include <utility>

#include "../datatypes/affix_fuzzer1.hpp"

namespace arrow {
    class DataType;
}

namespace rr {
    namespace components {
        struct AffixFuzzer4 {
            std::optional<rr::datatypes::AffixFuzzer1> single_optional;

          public:
            AffixFuzzer4(std::optional<rr::datatypes::AffixFuzzer1> single_optional)
                : single_optional(std::move(single_optional)) {}

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace components
} // namespace rr
