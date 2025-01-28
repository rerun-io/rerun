// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_line.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/aggregation_policy.hpp"
#include "../components/color.hpp"
#include "../components/name.hpp"
#include "../components/stroke_width.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Define the style properties for a line series in a chart.
    ///
    /// This archetype only provides styling information and should be logged as static
    /// when possible. The underlying data needs to be logged to the same entity-path using
    /// `archetypes::Scalar`.
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
    ///         rerun::SeriesLine().with_color({255, 0, 0}).with_name("sin(0.01t)").with_width(2)
    ///     );
    ///     rec.log_static(
    ///         "trig/cos",
    ///         rerun::SeriesLine().with_color({0, 255, 0}).with_name("cos(0.01t)").with_width(4)
    ///     );
    ///
    ///     // Log the data on a timeline called "step".
    ///     for (int t = 0; t <static_cast<int>(TAU * 2.0 * 100.0); ++t) {
    ///         rec.set_time_sequence("step", t);
    ///
    ///         rec.log("trig/sin", rerun::Scalar(sin(static_cast<double>(t) / 100.0)));
    ///         rec.log("trig/cos", rerun::Scalar(cos(static_cast<double>(t) / 100.0f)));
    ///     }
    /// }
    /// ```
    struct SeriesLine {
        /// Color for the corresponding series.
        std::optional<ComponentBatch> color;

        /// Stroke width for the corresponding series.
        std::optional<ComponentBatch> width;

        /// Display name of the series.
        ///
        /// Used in the legend.
        std::optional<ComponentBatch> name;

        /// Configures the zoom-dependent scalar aggregation.
        ///
        /// This is done only if steps on the X axis go below a single pixel,
        /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
        /// (and readability) in such situations as it prevents overdraw.
        std::optional<ComponentBatch> aggregation_policy;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SeriesLineIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.SeriesLine";

        /// `ComponentDescriptor` for the `color` field.
        static constexpr auto Descriptor_color = ComponentDescriptor(
            ArchetypeName, "color", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `width` field.
        static constexpr auto Descriptor_width = ComponentDescriptor(
            ArchetypeName, "width",
            Loggable<rerun::components::StrokeWidth>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `name` field.
        static constexpr auto Descriptor_name = ComponentDescriptor(
            ArchetypeName, "name", Loggable<rerun::components::Name>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `aggregation_policy` field.
        static constexpr auto Descriptor_aggregation_policy = ComponentDescriptor(
            ArchetypeName, "aggregation_policy",
            Loggable<rerun::components::AggregationPolicy>::Descriptor.component_name
        );

      public:
        SeriesLine() = default;
        SeriesLine(SeriesLine&& other) = default;
        SeriesLine(const SeriesLine& other) = default;
        SeriesLine& operator=(const SeriesLine& other) = default;
        SeriesLine& operator=(SeriesLine&& other) = default;

        /// Update only some specific fields of a `SeriesLine`.
        static SeriesLine update_fields() {
            return SeriesLine();
        }

        /// Clear all the fields of a `SeriesLine`.
        static SeriesLine clear_fields();

        /// Color for the corresponding series.
        SeriesLine with_color(const rerun::components::Color& _color) && {
            color = ComponentBatch::from_loggable(_color, Descriptor_color).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `color` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_color` should
        /// be used when logging a single row's worth of data.
        SeriesLine with_many_color(const Collection<rerun::components::Color>& _color) && {
            color = ComponentBatch::from_loggable(_color, Descriptor_color).value_or_throw();
            return std::move(*this);
        }

        /// Stroke width for the corresponding series.
        SeriesLine with_width(const rerun::components::StrokeWidth& _width) && {
            width = ComponentBatch::from_loggable(_width, Descriptor_width).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `width` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_width` should
        /// be used when logging a single row's worth of data.
        SeriesLine with_many_width(const Collection<rerun::components::StrokeWidth>& _width) && {
            width = ComponentBatch::from_loggable(_width, Descriptor_width).value_or_throw();
            return std::move(*this);
        }

        /// Display name of the series.
        ///
        /// Used in the legend.
        SeriesLine with_name(const rerun::components::Name& _name) && {
            name = ComponentBatch::from_loggable(_name, Descriptor_name).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `name` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_name` should
        /// be used when logging a single row's worth of data.
        SeriesLine with_many_name(const Collection<rerun::components::Name>& _name) && {
            name = ComponentBatch::from_loggable(_name, Descriptor_name).value_or_throw();
            return std::move(*this);
        }

        /// Configures the zoom-dependent scalar aggregation.
        ///
        /// This is done only if steps on the X axis go below a single pixel,
        /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
        /// (and readability) in such situations as it prevents overdraw.
        SeriesLine with_aggregation_policy(
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
        SeriesLine with_many_aggregation_policy(
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
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
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
    struct AsComponents<archetypes::SeriesLine> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::SeriesLine& archetype
        );
    };
} // namespace rerun
