// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <string>
#include <utility>

namespace rr {
    namespace components {
        struct AffixFuzzer9 {
            std::string single_string_required;

            AffixFuzzer9(std::string single_string_required)
                : single_string_required(std::move(single_string_required)) {}
        };
    } // namespace components
} // namespace rr
