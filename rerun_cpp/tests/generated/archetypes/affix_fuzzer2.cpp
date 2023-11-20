// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer2.hpp"

#include <rerun/collection_adapter_builtins.hpp>

namespace rerun::archetypes {
    const char AffixFuzzer2::INDICATOR_COMPONENT_NAME[] =
        "rerun.testing.components.AffixFuzzer2Indicator";
}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::AffixFuzzer2>::serialize(
        const archetypes::AffixFuzzer2& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(19);

        {
            auto result = rerun::components::AffixFuzzer1::to_data_cell(
                archetype.fuzz1101.data(),
                archetype.fuzz1101.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer2::to_data_cell(
                archetype.fuzz1102.data(),
                archetype.fuzz1102.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer3::to_data_cell(
                archetype.fuzz1103.data(),
                archetype.fuzz1103.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer4::to_data_cell(
                archetype.fuzz1104.data(),
                archetype.fuzz1104.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer5::to_data_cell(
                archetype.fuzz1105.data(),
                archetype.fuzz1105.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer6::to_data_cell(
                archetype.fuzz1106.data(),
                archetype.fuzz1106.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer7::to_data_cell(
                archetype.fuzz1107.data(),
                archetype.fuzz1107.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer8::to_data_cell(
                archetype.fuzz1108.data(),
                archetype.fuzz1108.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer9::to_data_cell(
                archetype.fuzz1109.data(),
                archetype.fuzz1109.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer10::to_data_cell(
                archetype.fuzz1110.data(),
                archetype.fuzz1110.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer11::to_data_cell(
                archetype.fuzz1111.data(),
                archetype.fuzz1111.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer12::to_data_cell(
                archetype.fuzz1112.data(),
                archetype.fuzz1112.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer13::to_data_cell(
                archetype.fuzz1113.data(),
                archetype.fuzz1113.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer14::to_data_cell(
                archetype.fuzz1114.data(),
                archetype.fuzz1114.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer15::to_data_cell(
                archetype.fuzz1115.data(),
                archetype.fuzz1115.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer16::to_data_cell(
                archetype.fuzz1116.data(),
                archetype.fuzz1116.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer17::to_data_cell(
                archetype.fuzz1117.data(),
                archetype.fuzz1117.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = rerun::components::AffixFuzzer18::to_data_cell(
                archetype.fuzz1118.data(),
                archetype.fuzz1118.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto indicator = AffixFuzzer2::IndicatorComponent();
            auto result = AffixFuzzer2::IndicatorComponent::to_data_cell(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
