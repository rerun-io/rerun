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
#include "../components/affix_fuzzer19.hpp"
#include "../components/affix_fuzzer2.hpp"
#include "../components/affix_fuzzer20.hpp"
#include "../components/affix_fuzzer21.hpp"
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
    struct AffixFuzzer1 {
        std::optional<ComponentBatch> fuzz1001;

        std::optional<ComponentBatch> fuzz1002;

        std::optional<ComponentBatch> fuzz1003;

        std::optional<ComponentBatch> fuzz1004;

        std::optional<ComponentBatch> fuzz1005;

        std::optional<ComponentBatch> fuzz1006;

        std::optional<ComponentBatch> fuzz1007;

        std::optional<ComponentBatch> fuzz1008;

        std::optional<ComponentBatch> fuzz1009;

        std::optional<ComponentBatch> fuzz1010;

        std::optional<ComponentBatch> fuzz1011;

        std::optional<ComponentBatch> fuzz1012;

        std::optional<ComponentBatch> fuzz1013;

        std::optional<ComponentBatch> fuzz1014;

        std::optional<ComponentBatch> fuzz1015;

        std::optional<ComponentBatch> fuzz1016;

        std::optional<ComponentBatch> fuzz1017;

        std::optional<ComponentBatch> fuzz1018;

        std::optional<ComponentBatch> fuzz1019;

        std::optional<ComponentBatch> fuzz1020;

        std::optional<ComponentBatch> fuzz1021;

        std::optional<ComponentBatch> fuzz1022;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.testing.components.AffixFuzzer1Indicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.testing.archetypes.AffixFuzzer1";

        /// `ComponentDescriptor` for the `fuzz1001` field.
        static constexpr auto Descriptor_fuzz1001 = ComponentDescriptor(
            ArchetypeName, "fuzz1001",
            Loggable<rerun::components::AffixFuzzer1>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1002` field.
        static constexpr auto Descriptor_fuzz1002 = ComponentDescriptor(
            ArchetypeName, "fuzz1002",
            Loggable<rerun::components::AffixFuzzer2>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1003` field.
        static constexpr auto Descriptor_fuzz1003 = ComponentDescriptor(
            ArchetypeName, "fuzz1003",
            Loggable<rerun::components::AffixFuzzer3>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1004` field.
        static constexpr auto Descriptor_fuzz1004 = ComponentDescriptor(
            ArchetypeName, "fuzz1004",
            Loggable<rerun::components::AffixFuzzer4>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1005` field.
        static constexpr auto Descriptor_fuzz1005 = ComponentDescriptor(
            ArchetypeName, "fuzz1005",
            Loggable<rerun::components::AffixFuzzer5>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1006` field.
        static constexpr auto Descriptor_fuzz1006 = ComponentDescriptor(
            ArchetypeName, "fuzz1006",
            Loggable<rerun::components::AffixFuzzer6>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1007` field.
        static constexpr auto Descriptor_fuzz1007 = ComponentDescriptor(
            ArchetypeName, "fuzz1007",
            Loggable<rerun::components::AffixFuzzer7>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1008` field.
        static constexpr auto Descriptor_fuzz1008 = ComponentDescriptor(
            ArchetypeName, "fuzz1008",
            Loggable<rerun::components::AffixFuzzer8>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1009` field.
        static constexpr auto Descriptor_fuzz1009 = ComponentDescriptor(
            ArchetypeName, "fuzz1009",
            Loggable<rerun::components::AffixFuzzer9>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1010` field.
        static constexpr auto Descriptor_fuzz1010 = ComponentDescriptor(
            ArchetypeName, "fuzz1010",
            Loggable<rerun::components::AffixFuzzer10>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1011` field.
        static constexpr auto Descriptor_fuzz1011 = ComponentDescriptor(
            ArchetypeName, "fuzz1011",
            Loggable<rerun::components::AffixFuzzer11>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1012` field.
        static constexpr auto Descriptor_fuzz1012 = ComponentDescriptor(
            ArchetypeName, "fuzz1012",
            Loggable<rerun::components::AffixFuzzer12>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1013` field.
        static constexpr auto Descriptor_fuzz1013 = ComponentDescriptor(
            ArchetypeName, "fuzz1013",
            Loggable<rerun::components::AffixFuzzer13>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1014` field.
        static constexpr auto Descriptor_fuzz1014 = ComponentDescriptor(
            ArchetypeName, "fuzz1014",
            Loggable<rerun::components::AffixFuzzer14>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1015` field.
        static constexpr auto Descriptor_fuzz1015 = ComponentDescriptor(
            ArchetypeName, "fuzz1015",
            Loggable<rerun::components::AffixFuzzer15>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1016` field.
        static constexpr auto Descriptor_fuzz1016 = ComponentDescriptor(
            ArchetypeName, "fuzz1016",
            Loggable<rerun::components::AffixFuzzer16>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1017` field.
        static constexpr auto Descriptor_fuzz1017 = ComponentDescriptor(
            ArchetypeName, "fuzz1017",
            Loggable<rerun::components::AffixFuzzer17>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1018` field.
        static constexpr auto Descriptor_fuzz1018 = ComponentDescriptor(
            ArchetypeName, "fuzz1018",
            Loggable<rerun::components::AffixFuzzer18>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1019` field.
        static constexpr auto Descriptor_fuzz1019 = ComponentDescriptor(
            ArchetypeName, "fuzz1019",
            Loggable<rerun::components::AffixFuzzer19>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1020` field.
        static constexpr auto Descriptor_fuzz1020 = ComponentDescriptor(
            ArchetypeName, "fuzz1020",
            Loggable<rerun::components::AffixFuzzer20>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1021` field.
        static constexpr auto Descriptor_fuzz1021 = ComponentDescriptor(
            ArchetypeName, "fuzz1021",
            Loggable<rerun::components::AffixFuzzer21>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fuzz1022` field.
        static constexpr auto Descriptor_fuzz1022 = ComponentDescriptor(
            ArchetypeName, "fuzz1022",
            Loggable<rerun::components::AffixFuzzer22>::Descriptor.component_name
        );

      public:
        AffixFuzzer1() = default;
        AffixFuzzer1(AffixFuzzer1&& other) = default;
        AffixFuzzer1(const AffixFuzzer1& other) = default;
        AffixFuzzer1& operator=(const AffixFuzzer1& other) = default;
        AffixFuzzer1& operator=(AffixFuzzer1&& other) = default;

        explicit AffixFuzzer1(
            rerun::components::AffixFuzzer1 _fuzz1001, rerun::components::AffixFuzzer2 _fuzz1002,
            rerun::components::AffixFuzzer3 _fuzz1003, rerun::components::AffixFuzzer4 _fuzz1004,
            rerun::components::AffixFuzzer5 _fuzz1005, rerun::components::AffixFuzzer6 _fuzz1006,
            rerun::components::AffixFuzzer7 _fuzz1007, rerun::components::AffixFuzzer8 _fuzz1008,
            rerun::components::AffixFuzzer9 _fuzz1009, rerun::components::AffixFuzzer10 _fuzz1010,
            rerun::components::AffixFuzzer11 _fuzz1011, rerun::components::AffixFuzzer12 _fuzz1012,
            rerun::components::AffixFuzzer13 _fuzz1013, rerun::components::AffixFuzzer14 _fuzz1014,
            rerun::components::AffixFuzzer15 _fuzz1015, rerun::components::AffixFuzzer16 _fuzz1016,
            rerun::components::AffixFuzzer17 _fuzz1017, rerun::components::AffixFuzzer18 _fuzz1018,
            rerun::components::AffixFuzzer19 _fuzz1019, rerun::components::AffixFuzzer20 _fuzz1020,
            rerun::components::AffixFuzzer21 _fuzz1021, rerun::components::AffixFuzzer22 _fuzz1022
        )
            : fuzz1001(ComponentBatch::from_loggable(std::move(_fuzz1001), Descriptor_fuzz1001)
                           .value_or_throw()),
              fuzz1002(ComponentBatch::from_loggable(std::move(_fuzz1002), Descriptor_fuzz1002)
                           .value_or_throw()),
              fuzz1003(ComponentBatch::from_loggable(std::move(_fuzz1003), Descriptor_fuzz1003)
                           .value_or_throw()),
              fuzz1004(ComponentBatch::from_loggable(std::move(_fuzz1004), Descriptor_fuzz1004)
                           .value_or_throw()),
              fuzz1005(ComponentBatch::from_loggable(std::move(_fuzz1005), Descriptor_fuzz1005)
                           .value_or_throw()),
              fuzz1006(ComponentBatch::from_loggable(std::move(_fuzz1006), Descriptor_fuzz1006)
                           .value_or_throw()),
              fuzz1007(ComponentBatch::from_loggable(std::move(_fuzz1007), Descriptor_fuzz1007)
                           .value_or_throw()),
              fuzz1008(ComponentBatch::from_loggable(std::move(_fuzz1008), Descriptor_fuzz1008)
                           .value_or_throw()),
              fuzz1009(ComponentBatch::from_loggable(std::move(_fuzz1009), Descriptor_fuzz1009)
                           .value_or_throw()),
              fuzz1010(ComponentBatch::from_loggable(std::move(_fuzz1010), Descriptor_fuzz1010)
                           .value_or_throw()),
              fuzz1011(ComponentBatch::from_loggable(std::move(_fuzz1011), Descriptor_fuzz1011)
                           .value_or_throw()),
              fuzz1012(ComponentBatch::from_loggable(std::move(_fuzz1012), Descriptor_fuzz1012)
                           .value_or_throw()),
              fuzz1013(ComponentBatch::from_loggable(std::move(_fuzz1013), Descriptor_fuzz1013)
                           .value_or_throw()),
              fuzz1014(ComponentBatch::from_loggable(std::move(_fuzz1014), Descriptor_fuzz1014)
                           .value_or_throw()),
              fuzz1015(ComponentBatch::from_loggable(std::move(_fuzz1015), Descriptor_fuzz1015)
                           .value_or_throw()),
              fuzz1016(ComponentBatch::from_loggable(std::move(_fuzz1016), Descriptor_fuzz1016)
                           .value_or_throw()),
              fuzz1017(ComponentBatch::from_loggable(std::move(_fuzz1017), Descriptor_fuzz1017)
                           .value_or_throw()),
              fuzz1018(ComponentBatch::from_loggable(std::move(_fuzz1018), Descriptor_fuzz1018)
                           .value_or_throw()),
              fuzz1019(ComponentBatch::from_loggable(std::move(_fuzz1019), Descriptor_fuzz1019)
                           .value_or_throw()),
              fuzz1020(ComponentBatch::from_loggable(std::move(_fuzz1020), Descriptor_fuzz1020)
                           .value_or_throw()),
              fuzz1021(ComponentBatch::from_loggable(std::move(_fuzz1021), Descriptor_fuzz1021)
                           .value_or_throw()),
              fuzz1022(ComponentBatch::from_loggable(std::move(_fuzz1022), Descriptor_fuzz1022)
                           .value_or_throw()) {}

        /// Update only some specific fields of a `AffixFuzzer1`.
        static AffixFuzzer1 update_fields() {
            return AffixFuzzer1();
        }

        /// Clear all the fields of a `AffixFuzzer1`.
        static AffixFuzzer1 clear_fields();

        AffixFuzzer1 with_fuzz1001(const rerun::components::AffixFuzzer1& _fuzz1001) && {
            fuzz1001 =
                ComponentBatch::from_loggable(_fuzz1001, Descriptor_fuzz1001).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1002(const rerun::components::AffixFuzzer2& _fuzz1002) && {
            fuzz1002 =
                ComponentBatch::from_loggable(_fuzz1002, Descriptor_fuzz1002).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1003(const rerun::components::AffixFuzzer3& _fuzz1003) && {
            fuzz1003 =
                ComponentBatch::from_loggable(_fuzz1003, Descriptor_fuzz1003).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1004(const rerun::components::AffixFuzzer4& _fuzz1004) && {
            fuzz1004 =
                ComponentBatch::from_loggable(_fuzz1004, Descriptor_fuzz1004).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1005(const rerun::components::AffixFuzzer5& _fuzz1005) && {
            fuzz1005 =
                ComponentBatch::from_loggable(_fuzz1005, Descriptor_fuzz1005).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1006(const rerun::components::AffixFuzzer6& _fuzz1006) && {
            fuzz1006 =
                ComponentBatch::from_loggable(_fuzz1006, Descriptor_fuzz1006).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1007(const rerun::components::AffixFuzzer7& _fuzz1007) && {
            fuzz1007 =
                ComponentBatch::from_loggable(_fuzz1007, Descriptor_fuzz1007).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1008(const rerun::components::AffixFuzzer8& _fuzz1008) && {
            fuzz1008 =
                ComponentBatch::from_loggable(_fuzz1008, Descriptor_fuzz1008).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1009(const rerun::components::AffixFuzzer9& _fuzz1009) && {
            fuzz1009 =
                ComponentBatch::from_loggable(_fuzz1009, Descriptor_fuzz1009).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1010(const rerun::components::AffixFuzzer10& _fuzz1010) && {
            fuzz1010 =
                ComponentBatch::from_loggable(_fuzz1010, Descriptor_fuzz1010).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1011(const rerun::components::AffixFuzzer11& _fuzz1011) && {
            fuzz1011 =
                ComponentBatch::from_loggable(_fuzz1011, Descriptor_fuzz1011).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1012(const rerun::components::AffixFuzzer12& _fuzz1012) && {
            fuzz1012 =
                ComponentBatch::from_loggable(_fuzz1012, Descriptor_fuzz1012).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1013(const rerun::components::AffixFuzzer13& _fuzz1013) && {
            fuzz1013 =
                ComponentBatch::from_loggable(_fuzz1013, Descriptor_fuzz1013).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1014(const rerun::components::AffixFuzzer14& _fuzz1014) && {
            fuzz1014 =
                ComponentBatch::from_loggable(_fuzz1014, Descriptor_fuzz1014).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1015(const rerun::components::AffixFuzzer15& _fuzz1015) && {
            fuzz1015 =
                ComponentBatch::from_loggable(_fuzz1015, Descriptor_fuzz1015).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1016(const rerun::components::AffixFuzzer16& _fuzz1016) && {
            fuzz1016 =
                ComponentBatch::from_loggable(_fuzz1016, Descriptor_fuzz1016).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1017(const rerun::components::AffixFuzzer17& _fuzz1017) && {
            fuzz1017 =
                ComponentBatch::from_loggable(_fuzz1017, Descriptor_fuzz1017).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1018(const rerun::components::AffixFuzzer18& _fuzz1018) && {
            fuzz1018 =
                ComponentBatch::from_loggable(_fuzz1018, Descriptor_fuzz1018).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1019(const rerun::components::AffixFuzzer19& _fuzz1019) && {
            fuzz1019 =
                ComponentBatch::from_loggable(_fuzz1019, Descriptor_fuzz1019).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1020(const rerun::components::AffixFuzzer20& _fuzz1020) && {
            fuzz1020 =
                ComponentBatch::from_loggable(_fuzz1020, Descriptor_fuzz1020).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1021(const rerun::components::AffixFuzzer21& _fuzz1021) && {
            fuzz1021 =
                ComponentBatch::from_loggable(_fuzz1021, Descriptor_fuzz1021).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer1 with_fuzz1022(const rerun::components::AffixFuzzer22& _fuzz1022) && {
            fuzz1022 =
                ComponentBatch::from_loggable(_fuzz1022, Descriptor_fuzz1022).value_or_throw();
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
    struct AsComponents<archetypes::AffixFuzzer1> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::AffixFuzzer1& archetype
        );
    };
} // namespace rerun
