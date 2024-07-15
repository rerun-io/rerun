// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/range_table_settings.fbs".

#include "table_row_order.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<blueprint::archetypes::TableRowOrder>::serialize(
        const blueprint::archetypes::TableRowOrder& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<DataCell> cells;
        cells.reserve(3);

        if (archetype.sort_key.has_value()) {
            auto result = DataCell::from_loggable(archetype.sort_key.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.sort_order.has_value()) {
            auto result = DataCell::from_loggable(archetype.sort_order.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = TableRowOrder::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
