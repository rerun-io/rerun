// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/dataframe_view_mode.fbs".

#pragma once

#include "../../blueprint/components/dataframe_view_mode.hpp"
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
    /// **Archetype**: Configuration for the dataframe view
    struct DataframeViewMode {
        /// The kind of table to display
        std::optional<rerun::blueprint::components::DataframeViewMode> mode;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.DataframeViewModeIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        DataframeViewMode() = default;
        DataframeViewMode(DataframeViewMode&& other) = default;

        /// The kind of table to display
        DataframeViewMode with_mode(rerun::blueprint::components::DataframeViewMode _mode) && {
            mode = std::move(_mode);
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
    struct AsComponents<blueprint::archetypes::DataframeViewMode> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::DataframeViewMode& archetype
        );
    };
} // namespace rerun
