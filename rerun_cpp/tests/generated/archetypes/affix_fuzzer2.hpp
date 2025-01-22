// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

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
#include <optional>
#include <rerun/collection.hpp>
#include <rerun/compiler_utils.hpp>
#include <rerun/component_batch.hpp>
#include <rerun/indicator_component.hpp>
#include <rerun/result.hpp>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    struct AffixFuzzer2 {
        std::optional<ComponentBatch> fuzz1101;

        std::optional<ComponentBatch> fuzz1102;

        std::optional<ComponentBatch> fuzz1103;

        std::optional<ComponentBatch> fuzz1104;

        std::optional<ComponentBatch> fuzz1105;

        std::optional<ComponentBatch> fuzz1106;

        std::optional<ComponentBatch> fuzz1107;

        std::optional<ComponentBatch> fuzz1108;

        std::optional<ComponentBatch> fuzz1109;

        std::optional<ComponentBatch> fuzz1110;

        std::optional<ComponentBatch> fuzz1111;

        std::optional<ComponentBatch> fuzz1112;

        std::optional<ComponentBatch> fuzz1113;

        std::optional<ComponentBatch> fuzz1114;

        std::optional<ComponentBatch> fuzz1115;

        std::optional<ComponentBatch> fuzz1116;

        std::optional<ComponentBatch> fuzz1117;

        std::optional<ComponentBatch> fuzz1118;

        std::optional<ComponentBatch> fuzz1122;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.testing.components.AffixFuzzer2Indicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.testing.archetypes.AffixFuzzer2";

        /// `ComponentDescriptor` for the `fuzz1101` field.
        static constexpr auto Descriptor_fuzz1101 = ComponentDescriptor(
            ArchetypeName, "fuzz1101",
            Loggable<rerun::components::AffixFuzzer1>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1102` field.
        static constexpr auto Descriptor_fuzz1102 = ComponentDescriptor(
            ArchetypeName, "fuzz1102",
            Loggable<rerun::components::AffixFuzzer2>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1103` field.
        static constexpr auto Descriptor_fuzz1103 = ComponentDescriptor(
            ArchetypeName, "fuzz1103",
            Loggable<rerun::components::AffixFuzzer3>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1104` field.
        static constexpr auto Descriptor_fuzz1104 = ComponentDescriptor(
            ArchetypeName, "fuzz1104",
            Loggable<rerun::components::AffixFuzzer4>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1105` field.
        static constexpr auto Descriptor_fuzz1105 = ComponentDescriptor(
            ArchetypeName, "fuzz1105",
            Loggable<rerun::components::AffixFuzzer5>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1106` field.
        static constexpr auto Descriptor_fuzz1106 = ComponentDescriptor(
            ArchetypeName, "fuzz1106",
            Loggable<rerun::components::AffixFuzzer6>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1107` field.
        static constexpr auto Descriptor_fuzz1107 = ComponentDescriptor(
            ArchetypeName, "fuzz1107",
            Loggable<rerun::components::AffixFuzzer7>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1108` field.
        static constexpr auto Descriptor_fuzz1108 = ComponentDescriptor(
            ArchetypeName, "fuzz1108",
            Loggable<rerun::components::AffixFuzzer8>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1109` field.
        static constexpr auto Descriptor_fuzz1109 = ComponentDescriptor(
            ArchetypeName, "fuzz1109",
            Loggable<rerun::components::AffixFuzzer9>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1110` field.
        static constexpr auto Descriptor_fuzz1110 = ComponentDescriptor(
            ArchetypeName, "fuzz1110",
            Loggable<rerun::components::AffixFuzzer10>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1111` field.
        static constexpr auto Descriptor_fuzz1111 = ComponentDescriptor(
            ArchetypeName, "fuzz1111",
            Loggable<rerun::components::AffixFuzzer11>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1112` field.
        static constexpr auto Descriptor_fuzz1112 = ComponentDescriptor(
            ArchetypeName, "fuzz1112",
            Loggable<rerun::components::AffixFuzzer12>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1113` field.
        static constexpr auto Descriptor_fuzz1113 = ComponentDescriptor(
            ArchetypeName, "fuzz1113",
            Loggable<rerun::components::AffixFuzzer13>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1114` field.
        static constexpr auto Descriptor_fuzz1114 = ComponentDescriptor(
            ArchetypeName, "fuzz1114",
            Loggable<rerun::components::AffixFuzzer14>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1115` field.
        static constexpr auto Descriptor_fuzz1115 = ComponentDescriptor(
            ArchetypeName, "fuzz1115",
            Loggable<rerun::components::AffixFuzzer15>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1116` field.
        static constexpr auto Descriptor_fuzz1116 = ComponentDescriptor(
            ArchetypeName, "fuzz1116",
            Loggable<rerun::components::AffixFuzzer16>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1117` field.
        static constexpr auto Descriptor_fuzz1117 = ComponentDescriptor(
            ArchetypeName, "fuzz1117",
            Loggable<rerun::components::AffixFuzzer17>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1118` field.
        static constexpr auto Descriptor_fuzz1118 = ComponentDescriptor(
            ArchetypeName, "fuzz1118",
            Loggable<rerun::components::AffixFuzzer18>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1122` field.
        static constexpr auto Descriptor_fuzz1122 = ComponentDescriptor(
            ArchetypeName, "fuzz1122",
            Loggable<rerun::components::AffixFuzzer22>::Descriptor.component_name
        );

      public:
        AffixFuzzer2() = default;
        AffixFuzzer2(AffixFuzzer2&& other) = default;
        AffixFuzzer2(const AffixFuzzer2& other) = default;
        AffixFuzzer2& operator=(const AffixFuzzer2& other) = default;
        AffixFuzzer2& operator=(AffixFuzzer2&& other) = default;

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
            : fuzz1101(ComponentBatch::from_loggable(std::move(_fuzz1101), Descriptor_fuzz1101)
                           .value_or_throw()),
              fuzz1102(ComponentBatch::from_loggable(std::move(_fuzz1102), Descriptor_fuzz1102)
                           .value_or_throw()),
              fuzz1103(ComponentBatch::from_loggable(std::move(_fuzz1103), Descriptor_fuzz1103)
                           .value_or_throw()),
              fuzz1104(ComponentBatch::from_loggable(std::move(_fuzz1104), Descriptor_fuzz1104)
                           .value_or_throw()),
              fuzz1105(ComponentBatch::from_loggable(std::move(_fuzz1105), Descriptor_fuzz1105)
                           .value_or_throw()),
              fuzz1106(ComponentBatch::from_loggable(std::move(_fuzz1106), Descriptor_fuzz1106)
                           .value_or_throw()),
              fuzz1107(ComponentBatch::from_loggable(std::move(_fuzz1107), Descriptor_fuzz1107)
                           .value_or_throw()),
              fuzz1108(ComponentBatch::from_loggable(std::move(_fuzz1108), Descriptor_fuzz1108)
                           .value_or_throw()),
              fuzz1109(ComponentBatch::from_loggable(std::move(_fuzz1109), Descriptor_fuzz1109)
                           .value_or_throw()),
              fuzz1110(ComponentBatch::from_loggable(std::move(_fuzz1110), Descriptor_fuzz1110)
                           .value_or_throw()),
              fuzz1111(ComponentBatch::from_loggable(std::move(_fuzz1111), Descriptor_fuzz1111)
                           .value_or_throw()),
              fuzz1112(ComponentBatch::from_loggable(std::move(_fuzz1112), Descriptor_fuzz1112)
                           .value_or_throw()),
              fuzz1113(ComponentBatch::from_loggable(std::move(_fuzz1113), Descriptor_fuzz1113)
                           .value_or_throw()),
              fuzz1114(ComponentBatch::from_loggable(std::move(_fuzz1114), Descriptor_fuzz1114)
                           .value_or_throw()),
              fuzz1115(ComponentBatch::from_loggable(std::move(_fuzz1115), Descriptor_fuzz1115)
                           .value_or_throw()),
              fuzz1116(ComponentBatch::from_loggable(std::move(_fuzz1116), Descriptor_fuzz1116)
                           .value_or_throw()),
              fuzz1117(ComponentBatch::from_loggable(std::move(_fuzz1117), Descriptor_fuzz1117)
                           .value_or_throw()),
              fuzz1118(ComponentBatch::from_loggable(std::move(_fuzz1118), Descriptor_fuzz1118)
                           .value_or_throw()),
              fuzz1122(ComponentBatch::from_loggable(std::move(_fuzz1122), Descriptor_fuzz1122)
                           .value_or_throw()) {}

        /// Update only some specific fields of a `AffixFuzzer2`.
        static AffixFuzzer2 update_fields() {
            return AffixFuzzer2();
        }

        /// Clear all the fields of a `AffixFuzzer2`.
        static AffixFuzzer2 clear_fields();

        AffixFuzzer2 with_fuzz1101(const Collection<rerun::components::AffixFuzzer1>& _fuzz1101
        ) && {
            fuzz1101 =
                ComponentBatch::from_loggable(_fuzz1101, Descriptor_fuzz1101).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1102(const Collection<rerun::components::AffixFuzzer2>& _fuzz1102
        ) && {
            fuzz1102 =
                ComponentBatch::from_loggable(_fuzz1102, Descriptor_fuzz1102).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1103(const Collection<rerun::components::AffixFuzzer3>& _fuzz1103
        ) && {
            fuzz1103 =
                ComponentBatch::from_loggable(_fuzz1103, Descriptor_fuzz1103).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1104(const Collection<rerun::components::AffixFuzzer4>& _fuzz1104
        ) && {
            fuzz1104 =
                ComponentBatch::from_loggable(_fuzz1104, Descriptor_fuzz1104).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1105(const Collection<rerun::components::AffixFuzzer5>& _fuzz1105
        ) && {
            fuzz1105 =
                ComponentBatch::from_loggable(_fuzz1105, Descriptor_fuzz1105).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1106(const Collection<rerun::components::AffixFuzzer6>& _fuzz1106
        ) && {
            fuzz1106 =
                ComponentBatch::from_loggable(_fuzz1106, Descriptor_fuzz1106).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1107(const Collection<rerun::components::AffixFuzzer7>& _fuzz1107
        ) && {
            fuzz1107 =
                ComponentBatch::from_loggable(_fuzz1107, Descriptor_fuzz1107).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1108(const Collection<rerun::components::AffixFuzzer8>& _fuzz1108
        ) && {
            fuzz1108 =
                ComponentBatch::from_loggable(_fuzz1108, Descriptor_fuzz1108).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1109(const Collection<rerun::components::AffixFuzzer9>& _fuzz1109
        ) && {
            fuzz1109 =
                ComponentBatch::from_loggable(_fuzz1109, Descriptor_fuzz1109).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1110(const Collection<rerun::components::AffixFuzzer10>& _fuzz1110
        ) && {
            fuzz1110 =
                ComponentBatch::from_loggable(_fuzz1110, Descriptor_fuzz1110).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1111(const Collection<rerun::components::AffixFuzzer11>& _fuzz1111
        ) && {
            fuzz1111 =
                ComponentBatch::from_loggable(_fuzz1111, Descriptor_fuzz1111).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1112(const Collection<rerun::components::AffixFuzzer12>& _fuzz1112
        ) && {
            fuzz1112 =
                ComponentBatch::from_loggable(_fuzz1112, Descriptor_fuzz1112).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1113(const Collection<rerun::components::AffixFuzzer13>& _fuzz1113
        ) && {
            fuzz1113 =
                ComponentBatch::from_loggable(_fuzz1113, Descriptor_fuzz1113).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1114(const Collection<rerun::components::AffixFuzzer14>& _fuzz1114
        ) && {
            fuzz1114 =
                ComponentBatch::from_loggable(_fuzz1114, Descriptor_fuzz1114).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1115(const Collection<rerun::components::AffixFuzzer15>& _fuzz1115
        ) && {
            fuzz1115 =
                ComponentBatch::from_loggable(_fuzz1115, Descriptor_fuzz1115).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1116(const Collection<rerun::components::AffixFuzzer16>& _fuzz1116
        ) && {
            fuzz1116 =
                ComponentBatch::from_loggable(_fuzz1116, Descriptor_fuzz1116).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1117(const Collection<rerun::components::AffixFuzzer17>& _fuzz1117
        ) && {
            fuzz1117 =
                ComponentBatch::from_loggable(_fuzz1117, Descriptor_fuzz1117).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1118(const Collection<rerun::components::AffixFuzzer18>& _fuzz1118
        ) && {
            fuzz1118 =
                ComponentBatch::from_loggable(_fuzz1118, Descriptor_fuzz1118).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer2 with_fuzz1122(const Collection<rerun::components::AffixFuzzer22>& _fuzz1122
        ) && {
            fuzz1122 =
                ComponentBatch::from_loggable(_fuzz1122, Descriptor_fuzz1122).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
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
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::AffixFuzzer2& archetype
        );
    };
} // namespace rerun
