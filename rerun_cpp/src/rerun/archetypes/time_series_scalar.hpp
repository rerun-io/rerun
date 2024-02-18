// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/time_series_scalar.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/color.hpp"
#include "../components/radius.hpp"
#include "../components/scalar.hpp"
#include "../components/scalar_scattering.hpp"
#include "../components/text.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Log a double-precision scalar that will be visualized as a time-series plot.
    ///
    /// The current simulation time will be used for the time/X-axis, hence scalars
    /// cannot be timeless!
    ///
    /// This archetype is in the process of being deprecated. Prefer usage of
    /// `Scalar`, `SeriesLine`, and `SeriesPoint` instead.
    ///
    /// See also `rerun::archetypes::Scalar`, `rerun::archetypes::SeriesPoint`, `rerun::archetypes::SeriesLine`.
    ///
    /// ## Example
    ///
    /// ### Simple line plot
    /// ![image](https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <cmath>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_scalar");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Log the data on a timeline called "step".
    ///     for (int step = 0; step <64; ++step) {
    ///         rec.set_time_sequence("step", step);
    ///         rec.log("scalar", rerun::Scalar(std::sin(static_cast<double>(step) / 10.0)));
    ///     }
    /// }
    /// ```
    struct [[deprecated(
        "Use the `Scalar` + (optional) `SeriesLine`/`SeriesPoint` archetypes instead, logged on the same entity. See [0.13 migration guide](https://www.rerun.io/docs/reference/migration/migration-0-13)."
    )]] TimeSeriesScalar {
        /// The scalar value to log.
        rerun::components::Scalar scalar;

        /// An optional radius for the point.
        ///
        /// Points within a single line do not have to share the same radius, the line
        /// will have differently sized segments as appropriate.
        ///
        /// If all points within a single entity path (i.e. a line) share the same
        /// radius, then this radius will be used as the line width too. Otherwise, the
        /// line will use the default width of `1.0`.
        std::optional<rerun::components::Radius> radius;

        /// Optional color for the scalar entry.
        ///
        /// If left unspecified, a pseudo-random color will be used instead. That
        /// same color will apply to all points residing in the same entity path
        /// that don't have a color specified.
        ///
        /// Points within a single line do not have to share the same color, the line
        /// will have differently colored segments as appropriate.
        /// If all points within a single entity path (i.e. a line) share the same
        /// color, then this color will be used as the line color in the plot legend.
        /// Otherwise, the line will appear gray in the legend.
        std::optional<rerun::components::Color> color;

        /// An optional label for the point.
        ///
        /// TODO(#1289): This won't show up on points at the moment, as our plots don't yet
        /// support displaying labels for individual points.
        /// If all points within a single entity path (i.e. a line) share the same label, then
        /// this label will be used as the label for the line itself. Otherwise, the
        /// line will be named after the entity path. The plot itself is named after
        /// the space it's in.
        std::optional<rerun::components::Text> label;

        /// Specifies whether a point in a scatter plot should form a continuous line.
        ///
        /// If set to true, this scalar will be drawn as a point, akin to a scatterplot.
        /// Otherwise, it will form a continuous line with its neighbors.
        /// Points within a single line do not have to all share the same scatteredness:
        /// the line will switch between a scattered and a continuous representation as
        /// required.
        std::optional<rerun::components::ScalarScattering> scattered;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.TimeSeriesScalarIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        TimeSeriesScalar() = default;
        TimeSeriesScalar(TimeSeriesScalar&& other) = default;

        explicit TimeSeriesScalar(rerun::components::Scalar _scalar) : scalar(std::move(_scalar)) {}

        /// An optional radius for the point.
        ///
        /// Points within a single line do not have to share the same radius, the line
        /// will have differently sized segments as appropriate.
        ///
        /// If all points within a single entity path (i.e. a line) share the same
        /// radius, then this radius will be used as the line width too. Otherwise, the
        /// line will use the default width of `1.0`.
        TimeSeriesScalar with_radius(rerun::components::Radius _radius) && {
            radius = std::move(_radius);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional color for the scalar entry.
        ///
        /// If left unspecified, a pseudo-random color will be used instead. That
        /// same color will apply to all points residing in the same entity path
        /// that don't have a color specified.
        ///
        /// Points within a single line do not have to share the same color, the line
        /// will have differently colored segments as appropriate.
        /// If all points within a single entity path (i.e. a line) share the same
        /// color, then this color will be used as the line color in the plot legend.
        /// Otherwise, the line will appear gray in the legend.
        TimeSeriesScalar with_color(rerun::components::Color _color) && {
            color = std::move(_color);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional label for the point.
        ///
        /// TODO(#1289): This won't show up on points at the moment, as our plots don't yet
        /// support displaying labels for individual points.
        /// If all points within a single entity path (i.e. a line) share the same label, then
        /// this label will be used as the label for the line itself. Otherwise, the
        /// line will be named after the entity path. The plot itself is named after
        /// the space it's in.
        TimeSeriesScalar with_label(rerun::components::Text _label) && {
            label = std::move(_label);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Specifies whether a point in a scatter plot should form a continuous line.
        ///
        /// If set to true, this scalar will be drawn as a point, akin to a scatterplot.
        /// Otherwise, it will form a continuous line with its neighbors.
        /// Points within a single line do not have to all share the same scatteredness:
        /// the line will switch between a scattered and a continuous representation as
        /// required.
        TimeSeriesScalar with_scattered(rerun::components::ScalarScattering _scattered) && {
            scattered = std::move(_scattered);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 1;
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;
    RR_PUSH_WARNINGS
    RR_DISABLE_DEPRECATION_WARNING

    /// \private
    template <>
    struct AsComponents<archetypes::TimeSeriesScalar> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::TimeSeriesScalar& archetype
        );
    };

    RR_POP_WARNINGS
} // namespace rerun
