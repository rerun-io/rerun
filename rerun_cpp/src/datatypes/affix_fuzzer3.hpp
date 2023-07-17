// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "../datatypes/affix_fuzzer1.hpp"

namespace rr {
    namespace datatypes {
        namespace detail {
            enum class AffixFuzzer3Tag {
                NONE = 0, // Makes it possible to implement move semantics
                degrees,
                radians,
                craziness,
                fixed_size_shenanigans,
            };

            union AffixFuzzer3Data {
                float degrees;

                std::optional<float> radians;

                std::vector<rr::datatypes::AffixFuzzer1> craziness;

                float fixed_size_shenanigans[3];

                AffixFuzzer3Data() {}

                ~AffixFuzzer3Data() {}
            };
        } // namespace detail

        struct AffixFuzzer3 {
          private:
            detail::AffixFuzzer3Tag _tag;
            detail::AffixFuzzer3Data _data;

            AffixFuzzer3() : _tag(detail::AffixFuzzer3Tag::NONE) {}

          public:
            ~AffixFuzzer3() {
                switch (this->_tag) {
                    case detail::AffixFuzzer3Tag::NONE: {
                        break; // Nothing to destroy
                    }
                    case detail::AffixFuzzer3Tag::degrees: {
                        break; // Plain Old Data (POD): requires no destructor
                    }
                    case detail::AffixFuzzer3Tag::radians: {
                        break; // Plain Old Data (POD): requires no destructor
                    }
                    case detail::AffixFuzzer3Tag::craziness: {
                        typedef std::vector<rr::datatypes::AffixFuzzer1> TypeAlias;
                        _data.craziness.~TypeAlias();
                        break;
                    }
                    case detail::AffixFuzzer3Tag::fixed_size_shenanigans: {
                        break; // Plain Old Data (POD): requires no destructor
                    }
                }
            }
        };
    } // namespace datatypes
} // namespace rr
