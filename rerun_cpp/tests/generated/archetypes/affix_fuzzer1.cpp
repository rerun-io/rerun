// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:53.
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer1.hpp"

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
#include "../components/affix_fuzzer3.hpp"
#include "../components/affix_fuzzer4.hpp"
#include "../components/affix_fuzzer5.hpp"
#include "../components/affix_fuzzer6.hpp"
#include "../components/affix_fuzzer7.hpp"
#include "../components/affix_fuzzer8.hpp"
#include "../components/affix_fuzzer9.hpp"

namespace rerun {
    namespace archetypes {
        Result<std::vector<rerun::DataCell>> AffixFuzzer1::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(74);

            {
                const auto result = rerun::components::AffixFuzzer1::to_data_cell(&fuzz1001, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer2::to_data_cell(&fuzz1002, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer3::to_data_cell(&fuzz1003, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer4::to_data_cell(&fuzz1004, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer5::to_data_cell(&fuzz1005, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer6::to_data_cell(&fuzz1006, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer7::to_data_cell(&fuzz1007, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer8::to_data_cell(&fuzz1008, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer9::to_data_cell(&fuzz1009, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer10::to_data_cell(&fuzz1010, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer11::to_data_cell(&fuzz1011, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer12::to_data_cell(&fuzz1012, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer13::to_data_cell(&fuzz1013, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer14::to_data_cell(&fuzz1014, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer15::to_data_cell(&fuzz1015, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer16::to_data_cell(&fuzz1016, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer17::to_data_cell(&fuzz1017, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer18::to_data_cell(&fuzz1018, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer19::to_data_cell(&fuzz1019, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer20::to_data_cell(&fuzz1020, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer1::to_data_cell(fuzz1101.data(), fuzz1101.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer2::to_data_cell(fuzz1102.data(), fuzz1102.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer3::to_data_cell(fuzz1103.data(), fuzz1103.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer4::to_data_cell(fuzz1104.data(), fuzz1104.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer5::to_data_cell(fuzz1105.data(), fuzz1105.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer6::to_data_cell(fuzz1106.data(), fuzz1106.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer7::to_data_cell(fuzz1107.data(), fuzz1107.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer8::to_data_cell(fuzz1108.data(), fuzz1108.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result =
                    rerun::components::AffixFuzzer9::to_data_cell(fuzz1109.data(), fuzz1109.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer10::to_data_cell(
                    fuzz1110.data(),
                    fuzz1110.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer11::to_data_cell(
                    fuzz1111.data(),
                    fuzz1111.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer12::to_data_cell(
                    fuzz1112.data(),
                    fuzz1112.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer13::to_data_cell(
                    fuzz1113.data(),
                    fuzz1113.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer14::to_data_cell(
                    fuzz1114.data(),
                    fuzz1114.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer15::to_data_cell(
                    fuzz1115.data(),
                    fuzz1115.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer16::to_data_cell(
                    fuzz1116.data(),
                    fuzz1116.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer17::to_data_cell(
                    fuzz1117.data(),
                    fuzz1117.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = rerun::components::AffixFuzzer18::to_data_cell(
                    fuzz1118.data(),
                    fuzz1118.size()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2001.has_value()) {
                const auto& value = fuzz2001.value();
                const auto result = rerun::components::AffixFuzzer1::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2002.has_value()) {
                const auto& value = fuzz2002.value();
                const auto result = rerun::components::AffixFuzzer2::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2003.has_value()) {
                const auto& value = fuzz2003.value();
                const auto result = rerun::components::AffixFuzzer3::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2004.has_value()) {
                const auto& value = fuzz2004.value();
                const auto result = rerun::components::AffixFuzzer4::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2005.has_value()) {
                const auto& value = fuzz2005.value();
                const auto result = rerun::components::AffixFuzzer5::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2006.has_value()) {
                const auto& value = fuzz2006.value();
                const auto result = rerun::components::AffixFuzzer6::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2007.has_value()) {
                const auto& value = fuzz2007.value();
                const auto result = rerun::components::AffixFuzzer7::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2008.has_value()) {
                const auto& value = fuzz2008.value();
                const auto result = rerun::components::AffixFuzzer8::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2009.has_value()) {
                const auto& value = fuzz2009.value();
                const auto result = rerun::components::AffixFuzzer9::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2010.has_value()) {
                const auto& value = fuzz2010.value();
                const auto result = rerun::components::AffixFuzzer10::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2011.has_value()) {
                const auto& value = fuzz2011.value();
                const auto result = rerun::components::AffixFuzzer11::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2012.has_value()) {
                const auto& value = fuzz2012.value();
                const auto result = rerun::components::AffixFuzzer12::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2013.has_value()) {
                const auto& value = fuzz2013.value();
                const auto result = rerun::components::AffixFuzzer13::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2014.has_value()) {
                const auto& value = fuzz2014.value();
                const auto result = rerun::components::AffixFuzzer14::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2015.has_value()) {
                const auto& value = fuzz2015.value();
                const auto result = rerun::components::AffixFuzzer15::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2016.has_value()) {
                const auto& value = fuzz2016.value();
                const auto result = rerun::components::AffixFuzzer16::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2017.has_value()) {
                const auto& value = fuzz2017.value();
                const auto result = rerun::components::AffixFuzzer17::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2018.has_value()) {
                const auto& value = fuzz2018.value();
                const auto result = rerun::components::AffixFuzzer18::to_data_cell(&value, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2101.has_value()) {
                const auto& value = fuzz2101.value();
                const auto result =
                    rerun::components::AffixFuzzer1::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2102.has_value()) {
                const auto& value = fuzz2102.value();
                const auto result =
                    rerun::components::AffixFuzzer2::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2103.has_value()) {
                const auto& value = fuzz2103.value();
                const auto result =
                    rerun::components::AffixFuzzer3::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2104.has_value()) {
                const auto& value = fuzz2104.value();
                const auto result =
                    rerun::components::AffixFuzzer4::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2105.has_value()) {
                const auto& value = fuzz2105.value();
                const auto result =
                    rerun::components::AffixFuzzer5::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2106.has_value()) {
                const auto& value = fuzz2106.value();
                const auto result =
                    rerun::components::AffixFuzzer6::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2107.has_value()) {
                const auto& value = fuzz2107.value();
                const auto result =
                    rerun::components::AffixFuzzer7::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2108.has_value()) {
                const auto& value = fuzz2108.value();
                const auto result =
                    rerun::components::AffixFuzzer8::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2109.has_value()) {
                const auto& value = fuzz2109.value();
                const auto result =
                    rerun::components::AffixFuzzer9::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2110.has_value()) {
                const auto& value = fuzz2110.value();
                const auto result =
                    rerun::components::AffixFuzzer10::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2111.has_value()) {
                const auto& value = fuzz2111.value();
                const auto result =
                    rerun::components::AffixFuzzer11::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2112.has_value()) {
                const auto& value = fuzz2112.value();
                const auto result =
                    rerun::components::AffixFuzzer12::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2113.has_value()) {
                const auto& value = fuzz2113.value();
                const auto result =
                    rerun::components::AffixFuzzer13::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2114.has_value()) {
                const auto& value = fuzz2114.value();
                const auto result =
                    rerun::components::AffixFuzzer14::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2115.has_value()) {
                const auto& value = fuzz2115.value();
                const auto result =
                    rerun::components::AffixFuzzer15::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2116.has_value()) {
                const auto& value = fuzz2116.value();
                const auto result =
                    rerun::components::AffixFuzzer16::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2117.has_value()) {
                const auto& value = fuzz2117.value();
                const auto result =
                    rerun::components::AffixFuzzer17::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            if (fuzz2118.has_value()) {
                const auto& value = fuzz2118.value();
                const auto result =
                    rerun::components::AffixFuzzer18::to_data_cell(value.data(), value.size());
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = create_indicator_component(
                    "rerun.components.AffixFuzzer1Indicator",
                    num_instances()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
