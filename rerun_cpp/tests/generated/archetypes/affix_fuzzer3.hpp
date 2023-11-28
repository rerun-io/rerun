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
#include <optional>
#include <rerun/collection.hpp>
#include <rerun/compiler_utils.hpp>
#include <rerun/data_cell.hpp>
#include <rerun/indicator_component.hpp>
#include <rerun/result.hpp>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    struct AffixFuzzer3 {
        std::optional<rerun::components::AffixFuzzer1> fuzz2001;

        std::optional<rerun::components::AffixFuzzer2> fuzz2002;

        std::optional<rerun::components::AffixFuzzer3> fuzz2003;

        std::optional<rerun::components::AffixFuzzer4> fuzz2004;

        std::optional<rerun::components::AffixFuzzer5> fuzz2005;

        std::optional<rerun::components::AffixFuzzer6> fuzz2006;

        std::optional<rerun::components::AffixFuzzer7> fuzz2007;

        std::optional<rerun::components::AffixFuzzer8> fuzz2008;

        std::optional<rerun::components::AffixFuzzer9> fuzz2009;

        std::optional<rerun::components::AffixFuzzer10> fuzz2010;

        std::optional<rerun::components::AffixFuzzer11> fuzz2011;

        std::optional<rerun::components::AffixFuzzer12> fuzz2012;

        std::optional<rerun::components::AffixFuzzer13> fuzz2013;

        std::optional<rerun::components::AffixFuzzer14> fuzz2014;

        std::optional<rerun::components::AffixFuzzer15> fuzz2015;

        std::optional<rerun::components::AffixFuzzer16> fuzz2016;

        std::optional<rerun::components::AffixFuzzer17> fuzz2017;

        std::optional<rerun::components::AffixFuzzer18> fuzz2018;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.testing.components.AffixFuzzer3Indicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = components::IndicatorComponent<IndicatorComponentName>;

      public:
        AffixFuzzer3() = default;
        AffixFuzzer3(AffixFuzzer3&& other) = default;

        AffixFuzzer3 with_fuzz2001(rerun::components::AffixFuzzer1 _fuzz2001) && {
            fuzz2001 = std::move(_fuzz2001);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2002(rerun::components::AffixFuzzer2 _fuzz2002) && {
            fuzz2002 = std::move(_fuzz2002);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2003(rerun::components::AffixFuzzer3 _fuzz2003) && {
            fuzz2003 = std::move(_fuzz2003);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2004(rerun::components::AffixFuzzer4 _fuzz2004) && {
            fuzz2004 = std::move(_fuzz2004);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2005(rerun::components::AffixFuzzer5 _fuzz2005) && {
            fuzz2005 = std::move(_fuzz2005);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2006(rerun::components::AffixFuzzer6 _fuzz2006) && {
            fuzz2006 = std::move(_fuzz2006);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2007(rerun::components::AffixFuzzer7 _fuzz2007) && {
            fuzz2007 = std::move(_fuzz2007);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2008(rerun::components::AffixFuzzer8 _fuzz2008) && {
            fuzz2008 = std::move(_fuzz2008);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2009(rerun::components::AffixFuzzer9 _fuzz2009) && {
            fuzz2009 = std::move(_fuzz2009);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2010(rerun::components::AffixFuzzer10 _fuzz2010) && {
            fuzz2010 = std::move(_fuzz2010);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2011(rerun::components::AffixFuzzer11 _fuzz2011) && {
            fuzz2011 = std::move(_fuzz2011);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2012(rerun::components::AffixFuzzer12 _fuzz2012) && {
            fuzz2012 = std::move(_fuzz2012);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2013(rerun::components::AffixFuzzer13 _fuzz2013) && {
            fuzz2013 = std::move(_fuzz2013);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2014(rerun::components::AffixFuzzer14 _fuzz2014) && {
            fuzz2014 = std::move(_fuzz2014);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2015(rerun::components::AffixFuzzer15 _fuzz2015) && {
            fuzz2015 = std::move(_fuzz2015);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2016(rerun::components::AffixFuzzer16 _fuzz2016) && {
            fuzz2016 = std::move(_fuzz2016);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2017(rerun::components::AffixFuzzer17 _fuzz2017) && {
            fuzz2017 = std::move(_fuzz2017);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        AffixFuzzer3 with_fuzz2018(rerun::components::AffixFuzzer18 _fuzz2018) && {
            fuzz2018 = std::move(_fuzz2018);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 0;
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::AffixFuzzer3> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::AffixFuzzer3& archetype);
    };
} // namespace rerun
