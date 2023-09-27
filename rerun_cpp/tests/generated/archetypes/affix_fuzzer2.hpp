// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#pragma once

#include "../components/affix_fuzzer1.hpp"
#include "../components/affix_fuzzer10.hpp"
#include "../components/affix_fuzzer11.hpp"
#include "../components/affix_fuzzer12.hpp"
#include "../components/affix_fuzzer13.hpp"
#include "../components/affix_fuzzer14.hpp"
#include "../components/affix_fuzzer15.hpp"
#include "../components/affix_fuzzer16.hpp"
#include "../components/affix_fuzzer17.hpp"
#include "../components/affix_fuzzer18.hpp"
#include "../components/affix_fuzzer2.hpp"
#include "../components/affix_fuzzer3.hpp"
#include "../components/affix_fuzzer4.hpp"
#include "../components/affix_fuzzer5.hpp"
#include "../components/affix_fuzzer6.hpp"
#include "../components/affix_fuzzer7.hpp"
#include "../components/affix_fuzzer8.hpp"
#include "../components/affix_fuzzer9.hpp"

#include <cstdint>
#include <rerun/arrow.hpp>
#include <rerun/component_batch.hpp>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        struct AffixFuzzer2 {
            std::vector<rerun::components::AffixFuzzer1> fuzz1101;

            std::vector<rerun::components::AffixFuzzer2> fuzz1102;

            std::vector<rerun::components::AffixFuzzer3> fuzz1103;

            std::vector<rerun::components::AffixFuzzer4> fuzz1104;

            std::vector<rerun::components::AffixFuzzer5> fuzz1105;

            std::vector<rerun::components::AffixFuzzer6> fuzz1106;

            std::vector<rerun::components::AffixFuzzer7> fuzz1107;

            std::vector<rerun::components::AffixFuzzer8> fuzz1108;

            std::vector<rerun::components::AffixFuzzer9> fuzz1109;

            std::vector<rerun::components::AffixFuzzer10> fuzz1110;

            std::vector<rerun::components::AffixFuzzer11> fuzz1111;

            std::vector<rerun::components::AffixFuzzer12> fuzz1112;

            std::vector<rerun::components::AffixFuzzer13> fuzz1113;

            std::vector<rerun::components::AffixFuzzer14> fuzz1114;

            std::vector<rerun::components::AffixFuzzer15> fuzz1115;

            std::vector<rerun::components::AffixFuzzer16> fuzz1116;

            std::vector<rerun::components::AffixFuzzer17> fuzz1117;

            std::vector<rerun::components::AffixFuzzer18> fuzz1118;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];

          public:
            AffixFuzzer2() = default;

            AffixFuzzer2(
                std::vector<rerun::components::AffixFuzzer1> _fuzz1101,
                std::vector<rerun::components::AffixFuzzer2> _fuzz1102,
                std::vector<rerun::components::AffixFuzzer3> _fuzz1103,
                std::vector<rerun::components::AffixFuzzer4> _fuzz1104,
                std::vector<rerun::components::AffixFuzzer5> _fuzz1105,
                std::vector<rerun::components::AffixFuzzer6> _fuzz1106,
                std::vector<rerun::components::AffixFuzzer7> _fuzz1107,
                std::vector<rerun::components::AffixFuzzer8> _fuzz1108,
                std::vector<rerun::components::AffixFuzzer9> _fuzz1109,
                std::vector<rerun::components::AffixFuzzer10> _fuzz1110,
                std::vector<rerun::components::AffixFuzzer11> _fuzz1111,
                std::vector<rerun::components::AffixFuzzer12> _fuzz1112,
                std::vector<rerun::components::AffixFuzzer13> _fuzz1113,
                std::vector<rerun::components::AffixFuzzer14> _fuzz1114,
                std::vector<rerun::components::AffixFuzzer15> _fuzz1115,
                std::vector<rerun::components::AffixFuzzer16> _fuzz1116,
                std::vector<rerun::components::AffixFuzzer17> _fuzz1117,
                std::vector<rerun::components::AffixFuzzer18> _fuzz1118
            )
                : fuzz1101(std::move(_fuzz1101)),
                  fuzz1102(std::move(_fuzz1102)),
                  fuzz1103(std::move(_fuzz1103)),
                  fuzz1104(std::move(_fuzz1104)),
                  fuzz1105(std::move(_fuzz1105)),
                  fuzz1106(std::move(_fuzz1106)),
                  fuzz1107(std::move(_fuzz1107)),
                  fuzz1108(std::move(_fuzz1108)),
                  fuzz1109(std::move(_fuzz1109)),
                  fuzz1110(std::move(_fuzz1110)),
                  fuzz1111(std::move(_fuzz1111)),
                  fuzz1112(std::move(_fuzz1112)),
                  fuzz1113(std::move(_fuzz1113)),
                  fuzz1114(std::move(_fuzz1114)),
                  fuzz1115(std::move(_fuzz1115)),
                  fuzz1116(std::move(_fuzz1116)),
                  fuzz1117(std::move(_fuzz1117)),
                  fuzz1118(std::move(_fuzz1118)) {}

            AffixFuzzer2(
                rerun::components::AffixFuzzer1 _fuzz1101,
                rerun::components::AffixFuzzer2 _fuzz1102,
                rerun::components::AffixFuzzer3 _fuzz1103,
                rerun::components::AffixFuzzer4 _fuzz1104,
                rerun::components::AffixFuzzer5 _fuzz1105,
                rerun::components::AffixFuzzer6 _fuzz1106,
                rerun::components::AffixFuzzer7 _fuzz1107,
                rerun::components::AffixFuzzer8 _fuzz1108,
                rerun::components::AffixFuzzer9 _fuzz1109,
                rerun::components::AffixFuzzer10 _fuzz1110,
                rerun::components::AffixFuzzer11 _fuzz1111,
                rerun::components::AffixFuzzer12 _fuzz1112,
                rerun::components::AffixFuzzer13 _fuzz1113,
                rerun::components::AffixFuzzer14 _fuzz1114,
                rerun::components::AffixFuzzer15 _fuzz1115,
                rerun::components::AffixFuzzer16 _fuzz1116,
                rerun::components::AffixFuzzer17 _fuzz1117,
                rerun::components::AffixFuzzer18 _fuzz1118
            )
                : fuzz1101(1, std::move(_fuzz1101)),
                  fuzz1102(1, std::move(_fuzz1102)),
                  fuzz1103(1, std::move(_fuzz1103)),
                  fuzz1104(1, std::move(_fuzz1104)),
                  fuzz1105(1, std::move(_fuzz1105)),
                  fuzz1106(1, std::move(_fuzz1106)),
                  fuzz1107(1, std::move(_fuzz1107)),
                  fuzz1108(1, std::move(_fuzz1108)),
                  fuzz1109(1, std::move(_fuzz1109)),
                  fuzz1110(1, std::move(_fuzz1110)),
                  fuzz1111(1, std::move(_fuzz1111)),
                  fuzz1112(1, std::move(_fuzz1112)),
                  fuzz1113(1, std::move(_fuzz1113)),
                  fuzz1114(1, std::move(_fuzz1114)),
                  fuzz1115(1, std::move(_fuzz1115)),
                  fuzz1116(1, std::move(_fuzz1116)),
                  fuzz1117(1, std::move(_fuzz1117)),
                  fuzz1118(1, std::move(_fuzz1118)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return fuzz1101.size();
            }

            /// Creates an `AnonymousComponentBatch` out of the associated indicator component. This
            /// allows for associating arbitrary indicator components with arbitrary data. Check out
            /// the `manual_indicator` API example to see what's possible.
            static AnonymousComponentBatch indicator();

            /// Collections all component lists into a list of component collections. *Attention:*
            /// The returned vector references this instance and does not take ownership of any
            /// data. Adding any new components to this archetype will invalidate the returned
            /// component lists!
            std::vector<AnonymousComponentBatch> as_component_batches() const;
        };
    } // namespace archetypes
} // namespace rerun
