// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <utility>

#include "../datatypes/affix_fuzzer3.hpp"

namespace rr {
    namespace components {
        struct AffixFuzzer14 {
            rr::datatypes::AffixFuzzer3 single_required_union;

            AffixFuzzer14(rr::datatypes::AffixFuzzer3 single_required_union)
                : single_required_union(std::move(single_required_union)) {}
        };
    } // namespace components
} // namespace rr
