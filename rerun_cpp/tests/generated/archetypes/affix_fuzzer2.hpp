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
#include "../components/affix_fuzzer22.hpp"
#include "../components/affix_fuzzer3.hpp"
#include "../components/affix_fuzzer4.hpp"
#include "../components/affix_fuzzer5.hpp"
#include "../components/affix_fuzzer6.hpp"
#include "../components/affix_fuzzer7.hpp"
#include "../components/affix_fuzzer8.hpp"
#include "../components/affix_fuzzer9.hpp"

#include <cstdint>
#include <rerun/collection.hpp>
#include <rerun/data_cell.hpp>
#include <rerun/indicator_component.hpp>
#include <rerun/result.hpp>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    struct AffixFuzzer2 {
        Collection<rerun::components::AffixFuzzer1> fuzz1101;

        Collection<rerun::components::AffixFuzzer2> fuzz1102;

        Collection<rerun::components::AffixFuzzer3> fuzz1103;

        Collection<rerun::components::AffixFuzzer4> fuzz1104;

        Collection<rerun::components::AffixFuzzer5> fuzz1105;

        Collection<rerun::components::AffixFuzzer6> fuzz1106;

        Collection<rerun::components::AffixFuzzer7> fuzz1107;

        Collection<rerun::components::AffixFuzzer8> fuzz1108;

        Collection<rerun::components::AffixFuzzer9> fuzz1109;

        Collection<rerun::components::AffixFuzzer10> fuzz1110;

        Collection<rerun::components::AffixFuzzer11> fuzz1111;

        Collection<rerun::components::AffixFuzzer12> fuzz1112;

        Collection<rerun::components::AffixFuzzer13> fuzz1113;

        Collection<rerun::components::AffixFuzzer14> fuzz1114;

        Collection<rerun::components::AffixFuzzer15> fuzz1115;

        Collection<rerun::components::AffixFuzzer16> fuzz1116;

        Collection<rerun::components::AffixFuzzer17> fuzz1117;

        Collection<rerun::components::AffixFuzzer18> fuzz1118;

        Collection<rerun::components::AffixFuzzer22> fuzz1122;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.testing.components.AffixFuzzer2Indicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        AffixFuzzer2() = default;
        AffixFuzzer2(AffixFuzzer2&& other) = default;

        explicit AffixFuzzer2(
            Collection<rerun::components::AffixFuzzer1> _fuzz1101,
            Collection<rerun::components::AffixFuzzer2> _fuzz1102,
            Collection<rerun::components::AffixFuzzer3> _fuzz1103,
            Collection<rerun::components::AffixFuzzer4> _fuzz1104,
            Collection<rerun::components::AffixFuzzer5> _fuzz1105,
            Collection<rerun::components::AffixFuzzer6> _fuzz1106,
            Collection<rerun::components::AffixFuzzer7> _fuzz1107,
            Collection<rerun::components::AffixFuzzer8> _fuzz1108,
            Collection<rerun::components::AffixFuzzer9> _fuzz1109,
            Collection<rerun::components::AffixFuzzer10> _fuzz1110,
            Collection<rerun::components::AffixFuzzer11> _fuzz1111,
            Collection<rerun::components::AffixFuzzer12> _fuzz1112,
            Collection<rerun::components::AffixFuzzer13> _fuzz1113,
            Collection<rerun::components::AffixFuzzer14> _fuzz1114,
            Collection<rerun::components::AffixFuzzer15> _fuzz1115,
            Collection<rerun::components::AffixFuzzer16> _fuzz1116,
            Collection<rerun::components::AffixFuzzer17> _fuzz1117,
            Collection<rerun::components::AffixFuzzer18> _fuzz1118,
            Collection<rerun::components::AffixFuzzer22> _fuzz1122
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
              fuzz1118(std::move(_fuzz1118)),
              fuzz1122(std::move(_fuzz1122)) {}

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return fuzz1101.size();
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::AffixFuzzer2> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::AffixFuzzer2& archetype);
    };
} // namespace rerun
