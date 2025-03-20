// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_lines.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/aggregation_policy.hpp"
#include "../components/color.hpp"
#include "../components/name.hpp"
#include "../components/series_visible.hpp"
#include "../components/stroke_width.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Define the style properties for one or more line series in a chart.
    ///
    /// This archetype only provides styling information.
    /// Changes over time are supported for most but not all its fields (see respective fields for details),
    /// it's generally recommended to log this type as static.
    ///
    /// The underlying data needs to be logged to the same entity-path using `archetypes::Scalars`.
    /// Dimensionality of the scalar arrays logged at each time point is assumed to be the same over time.
    ///
    /// ## Example
    ///
    /// ### Line series
    /// ![image](https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <cmath>
    ///
    /// constexpr float TAU = 6.28318530717958647692528676655900577f;
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_series_line_style");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Set up plot styling:
    ///     // They are logged static as they don't change over time and apply to all timelines.
    ///     // Log two lines series under a shared root so that they show in the same plot by default.
    ///     rec.log_static(
    ///         "trig/sin",
    ///         rerun::SeriesLines().with_colors({255, 0, 0}).with_names("sin(0.01t)").with_widths(2.0f)
    ///     );
    ///     rec.log_static(
    ///         "trig/cos",
    ///         rerun::SeriesLines().with_colors({0, 255, 0}).with_names("cos(0.01t)").with_widths(4.0f)
    ///     );
    ///
    ///     // Log the data on a timeline called "step".
    ///     for (int t = 0; t <static_cast<int>(TAU * 2.0 * 100.0); ++t) {
    ///         rec.set_time_sequence("step", t);
    ///
    ///         rec.log("trig/sin", rerun::Scalars(sin(static_cast<double>(t) / 100.0)));
    ///         rec.log("trig/cos", rerun::Scalars(cos(static_cast<double>(t) / 100.0)));
    ///     }
    /// }
    /// ```
    struct SeriesLines {
        /// Color for the corresponding series.
        ///
        /// May change over time, but can cause discontinuities in the line.
        std::optional<ComponentBatch> colors;

        /// Stroke width for the corresponding series.
        ///
        /// May change over time, but can cause discontinuities in the line.
        std::optional<ComponentBatch> widths;

        /// Display name of the series.
        ///
        /// Used in the legend. Expected to be unchanging over time.
        std::optional<ComponentBatch> names;

        /// Which lines are visible.
        ///
        /// If not set, all line series on this entity are visible.
        /// Unlike with the regular visibility property of the entire entity, any series that is hidden
        /// via this property will still be visible in the legend.
        ///
        /// May change over time, but can cause discontinuities in the line.
        std::optional<ComponentBatch> visible_series;

        /// Configures the zoom-dependent scalar aggregation.
        ///
        /// This is done only if steps on the X axis go below a single pixel,
        /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
        /// (and readability) in such situations as it prevents overdraw.
        ///
        /// Expected to be unchanging over time.
        std::optional<ComponentBatch> aggregation_policy;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SeriesLinesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.SeriesLines";

        /// `ComponentDescriptor` for the `colors` field.
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `widths` field.
        static constexpr auto Descriptor_widths = ComponentDescriptor(
            ArchetypeName, "widths",
            Loggable<rerun::components::StrokeWidth>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `names` field.
        static constexpr auto Descriptor_names = ComponentDescriptor(
            ArchetypeName, "names", Loggable<rerun::components::Name>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `visible_series` field.
        static constexpr auto Descriptor_visible_series = ComponentDescriptor(
            ArchetypeName, "visible_series",
            Loggable<rerun::components::SeriesVisible>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `aggregation_policy` field.
        static constexpr auto Descriptor_aggregation_policy = ComponentDescriptor(
            ArchetypeName, "aggregation_policy",
            Loggable<rerun::components::AggregationPolicy>::Descriptor.component_name
        );

      public: // START of extensions from series_lines_ext.cpp:
        // Overload needed to avoid confusion with passing single strings.
        /// Display name of the series.
        ///
        /// Used in the legend. Expected to be unchanging over time.
        SeriesLines with_names(const char* _name) && {
            names = ComponentBatch::from_loggable(rerun::components::Name(_name), Descriptor_names)
                        .value_or_throw();
            return std::move(*this);
        }

        // END of extensions from series_lines_ext.cpp, start of generated code:

      public:
        SeriesLines() = default;
        SeriesLines(SeriesLines&& other) = default;
        SeriesLines(const SeriesLines& other) = default;
        SeriesLines& operator=(const SeriesLines& other) = default;
        SeriesLines& operator=(SeriesLines&& other) = default;

        /// Update only some specific fields of a `SeriesLines`.
        static SeriesLines update_fields() {
            return SeriesLines();
        }

        /// Clear all the fields of a `SeriesLines`.
        static SeriesLines clear_fields();

        /// Color for the corresponding series.
        ///
        /// May change over time, but can cause discontinuities in the line.
        SeriesLines with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Stroke width for the corresponding series.
        ///
        /// May change over time, but can cause discontinuities in the line.
        SeriesLines with_widths(const Collection<rerun::components::StrokeWidth>& _widths) && {
            widths = ComponentBatch::from_loggable(_widths, Descriptor_widths).value_or_throw();
            return std::move(*this);
        }

        /// Display name of the series.
        ///
        /// Used in the legend. Expected to be unchanging over time.
        SeriesLines with_names(const Collection<rerun::components::Name>& _names) && {
            names = ComponentBatch::from_loggable(_names, Descriptor_names).value_or_throw();
            return std::move(*this);
        }

        /// Which lines are visible.
        ///
        /// If not set, all line series on this entity are visible.
        /// Unlike with the regular visibility property of the entire entity, any series that is hidden
        /// via this property will still be visible in the legend.
        ///
        /// May change over time, but can cause discontinuities in the line.
        SeriesLines with_visible_series(
            const Collection<rerun::components::SeriesVisible>& _visible_series
        ) && {
            visible_series =
                ComponentBatch::from_loggable(_visible_series, Descriptor_visible_series)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Configures the zoom-dependent scalar aggregation.
        ///
        /// This is done only if steps on the X axis go below a single pixel,
        /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
        /// (and readability) in such situations as it prevents overdraw.
        ///
        /// Expected to be unchanging over time.
        SeriesLines with_aggregation_policy(
            const rerun::components::AggregationPolicy& _aggregation_policy
        ) && {
            aggregation_policy =
                ComponentBatch::from_loggable(_aggregation_policy, Descriptor_aggregation_policy)
                    .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `aggregation_policy` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_aggregation_policy` should
        /// be used when logging a single row's worth of data.
        SeriesLines with_many_aggregation_policy(
            const Collection<rerun::components::AggregationPolicy>& _aggregation_policy
        ) && {
            aggregation_policy =
                ComponentBatch::from_loggable(_aggregation_policy, Descriptor_aggregation_policy)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentBatch::partitioned`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::SeriesLines> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const archetypes::SeriesLines& archetype
        );
    };
} // namespace rerun
