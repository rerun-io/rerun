// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/range_table_settings.fbs".

#pragma once

#include "../../blueprint/components/sort_order.hpp"
#include "../../blueprint/components/table_group_by.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the sorting of the rows of a time range table.
    struct TableRowOrder {
        /// The type of the background.
        std::optional<rerun::blueprint::components::TableGroupBy> group_by;

        /// Color used for the `SolidColor` background type.
        std::optional<rerun::blueprint::components::SortOrder> sort_order;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.TableRowOrderIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        TableRowOrder() = default;
        TableRowOrder(TableRowOrder&& other) = default;

        /// The type of the background.
        TableRowOrder with_group_by(rerun::blueprint::components::TableGroupBy _group_by) && {
            group_by = std::move(_group_by);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Color used for the `SolidColor` background type.
        TableRowOrder with_sort_order(rerun::blueprint::components::SortOrder _sort_order) && {
            sort_order = std::move(_sort_order);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::TableRowOrder> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::TableRowOrder& archetype
        );
    };
} // namespace rerun
