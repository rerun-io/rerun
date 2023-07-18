// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <utility>

#include "../datatypes/affix_fuzzer1.hpp"

namespace rr {
    namespace components {
        struct AffixFuzzer3 {
            rr::datatypes::AffixFuzzer1 single_required;

            AffixFuzzer3(rr::datatypes::AffixFuzzer1 single_required)
                : single_required(std::move(single_required)) {}
        };
    } // namespace components
} // namespace rr
