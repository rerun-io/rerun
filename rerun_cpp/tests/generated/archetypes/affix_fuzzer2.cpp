// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer2.hpp"

namespace rerun {
    namespace archetypes {
        const char AffixFuzzer2::INDICATOR_COMPONENT_NAME[] =
            "rerun.testing.components.AffixFuzzer2Indicator";

        Result<std::vector<SerializedComponentBatch>> AffixFuzzer2::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(18);

            {
                auto result = fuzz1101.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1102.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1103.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1104.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1105.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1106.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1107.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1108.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1109.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1110.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1111.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1112.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1113.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1114.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1115.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1116.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1117.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = fuzz1118.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = ComponentBatch<IndicatorComponent>(IndicatorComponent()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
