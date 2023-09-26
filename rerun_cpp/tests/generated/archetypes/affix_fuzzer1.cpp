// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer1.hpp"

#include <rerun/indicator_component.hpp>

namespace rerun {
    namespace archetypes {
        const char AffixFuzzer1::INDICATOR_COMPONENT_NAME[] =
            "rerun.testing.components.AffixFuzzer1Indicator";

        AnonymousComponentBatch AffixFuzzer1::indicator() {
            return ComponentBatch<
                components::IndicatorComponent<AffixFuzzer1::INDICATOR_COMPONENT_NAME>>(nullptr, 1);
        }

        std::vector<AnonymousComponentBatch> AffixFuzzer1::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(75);

            comp_batches.emplace_back(fuzz1001);
            comp_batches.emplace_back(fuzz1002);
            comp_batches.emplace_back(fuzz1003);
            comp_batches.emplace_back(fuzz1004);
            comp_batches.emplace_back(fuzz1005);
            comp_batches.emplace_back(fuzz1006);
            comp_batches.emplace_back(fuzz1007);
            comp_batches.emplace_back(fuzz1008);
            comp_batches.emplace_back(fuzz1009);
            comp_batches.emplace_back(fuzz1010);
            comp_batches.emplace_back(fuzz1011);
            comp_batches.emplace_back(fuzz1012);
            comp_batches.emplace_back(fuzz1013);
            comp_batches.emplace_back(fuzz1014);
            comp_batches.emplace_back(fuzz1015);
            comp_batches.emplace_back(fuzz1016);
            comp_batches.emplace_back(fuzz1017);
            comp_batches.emplace_back(fuzz1018);
            comp_batches.emplace_back(fuzz1019);
            comp_batches.emplace_back(fuzz1020);
            comp_batches.emplace_back(fuzz1021);
            comp_batches.emplace_back(fuzz1101);
            comp_batches.emplace_back(fuzz1102);
            comp_batches.emplace_back(fuzz1103);
            comp_batches.emplace_back(fuzz1104);
            comp_batches.emplace_back(fuzz1105);
            comp_batches.emplace_back(fuzz1106);
            comp_batches.emplace_back(fuzz1107);
            comp_batches.emplace_back(fuzz1108);
            comp_batches.emplace_back(fuzz1109);
            comp_batches.emplace_back(fuzz1110);
            comp_batches.emplace_back(fuzz1111);
            comp_batches.emplace_back(fuzz1112);
            comp_batches.emplace_back(fuzz1113);
            comp_batches.emplace_back(fuzz1114);
            comp_batches.emplace_back(fuzz1115);
            comp_batches.emplace_back(fuzz1116);
            comp_batches.emplace_back(fuzz1117);
            comp_batches.emplace_back(fuzz1118);
            if (fuzz2001.has_value()) {
                comp_batches.emplace_back(fuzz2001.value());
            }
            if (fuzz2002.has_value()) {
                comp_batches.emplace_back(fuzz2002.value());
            }
            if (fuzz2003.has_value()) {
                comp_batches.emplace_back(fuzz2003.value());
            }
            if (fuzz2004.has_value()) {
                comp_batches.emplace_back(fuzz2004.value());
            }
            if (fuzz2005.has_value()) {
                comp_batches.emplace_back(fuzz2005.value());
            }
            if (fuzz2006.has_value()) {
                comp_batches.emplace_back(fuzz2006.value());
            }
            if (fuzz2007.has_value()) {
                comp_batches.emplace_back(fuzz2007.value());
            }
            if (fuzz2008.has_value()) {
                comp_batches.emplace_back(fuzz2008.value());
            }
            if (fuzz2009.has_value()) {
                comp_batches.emplace_back(fuzz2009.value());
            }
            if (fuzz2010.has_value()) {
                comp_batches.emplace_back(fuzz2010.value());
            }
            if (fuzz2011.has_value()) {
                comp_batches.emplace_back(fuzz2011.value());
            }
            if (fuzz2012.has_value()) {
                comp_batches.emplace_back(fuzz2012.value());
            }
            if (fuzz2013.has_value()) {
                comp_batches.emplace_back(fuzz2013.value());
            }
            if (fuzz2014.has_value()) {
                comp_batches.emplace_back(fuzz2014.value());
            }
            if (fuzz2015.has_value()) {
                comp_batches.emplace_back(fuzz2015.value());
            }
            if (fuzz2016.has_value()) {
                comp_batches.emplace_back(fuzz2016.value());
            }
            if (fuzz2017.has_value()) {
                comp_batches.emplace_back(fuzz2017.value());
            }
            if (fuzz2018.has_value()) {
                comp_batches.emplace_back(fuzz2018.value());
            }
            if (fuzz2101.has_value()) {
                comp_batches.emplace_back(fuzz2101.value());
            }
            if (fuzz2102.has_value()) {
                comp_batches.emplace_back(fuzz2102.value());
            }
            if (fuzz2103.has_value()) {
                comp_batches.emplace_back(fuzz2103.value());
            }
            if (fuzz2104.has_value()) {
                comp_batches.emplace_back(fuzz2104.value());
            }
            if (fuzz2105.has_value()) {
                comp_batches.emplace_back(fuzz2105.value());
            }
            if (fuzz2106.has_value()) {
                comp_batches.emplace_back(fuzz2106.value());
            }
            if (fuzz2107.has_value()) {
                comp_batches.emplace_back(fuzz2107.value());
            }
            if (fuzz2108.has_value()) {
                comp_batches.emplace_back(fuzz2108.value());
            }
            if (fuzz2109.has_value()) {
                comp_batches.emplace_back(fuzz2109.value());
            }
            if (fuzz2110.has_value()) {
                comp_batches.emplace_back(fuzz2110.value());
            }
            if (fuzz2111.has_value()) {
                comp_batches.emplace_back(fuzz2111.value());
            }
            if (fuzz2112.has_value()) {
                comp_batches.emplace_back(fuzz2112.value());
            }
            if (fuzz2113.has_value()) {
                comp_batches.emplace_back(fuzz2113.value());
            }
            if (fuzz2114.has_value()) {
                comp_batches.emplace_back(fuzz2114.value());
            }
            if (fuzz2115.has_value()) {
                comp_batches.emplace_back(fuzz2115.value());
            }
            if (fuzz2116.has_value()) {
                comp_batches.emplace_back(fuzz2116.value());
            }
            if (fuzz2117.has_value()) {
                comp_batches.emplace_back(fuzz2117.value());
            }
            if (fuzz2118.has_value()) {
                comp_batches.emplace_back(fuzz2118.value());
            }
            comp_batches.emplace_back(AffixFuzzer1::indicator());

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
