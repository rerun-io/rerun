// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <utility>

namespace rr {
    namespace components {
        struct AffixFuzzer10 {
            std::optional<std::string> single_string_optional;

            AffixFuzzer10(std::optional<std::string> single_string_optional)
                : single_string_optional(std::move(single_string_optional)) {}
        };
    } // namespace components
} // namespace rr
