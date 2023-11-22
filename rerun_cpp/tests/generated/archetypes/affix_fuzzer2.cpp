// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer2.hpp"

#include <rerun/collection_adapter_builtins.hpp>

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::AffixFuzzer2>::serialize(
        const archetypes::AffixFuzzer2& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(19);

        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer1>(archetype.fuzz1101);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer2>(archetype.fuzz1102);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer3>(archetype.fuzz1103);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer4>(archetype.fuzz1104);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer5>(archetype.fuzz1105);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer6>(archetype.fuzz1106);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer7>(archetype.fuzz1107);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer8>(archetype.fuzz1108);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer9>(archetype.fuzz1109);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer10>(archetype.fuzz1110);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer11>(archetype.fuzz1111);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer12>(archetype.fuzz1112);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer13>(archetype.fuzz1113);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer14>(archetype.fuzz1114);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer15>(archetype.fuzz1115);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer16>(archetype.fuzz1116);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer17>(archetype.fuzz1117);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result =
                DataCell::from_loggable<rerun::components::AffixFuzzer18>(archetype.fuzz1118);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = AffixFuzzer2::IndicatorComponent();
            auto result = Loggable<AffixFuzzer2::IndicatorComponent>::to_arrow(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
