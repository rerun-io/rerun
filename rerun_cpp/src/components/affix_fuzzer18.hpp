// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <optional>
#include <vector>

namespace rr {
    namespace components {
        struct AffixFuzzer18 {
            std::optional<std::vector<rr::datatypes::AffixFuzzer4>> many_optional_unions;
        };
    } // namespace components
} // namespace rr
