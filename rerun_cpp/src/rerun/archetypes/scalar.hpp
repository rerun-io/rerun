// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/scalar.fbs".

#pragma once

#include "../collection.hpp"
#include "../components/scalar.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A double-precision scalar, e.g. for use for time-series plots.
    ///
    /// The current timeline value will be used for the time/X-axis, hence scalars
    /// cannot be static.
    ///
    /// When used to produce a plot, this archetype is used to provide the data that
    /// is referenced by `archetypes::SeriesLine` or `archetypes::SeriesPoint`. You can do
    /// this by logging both archetypes to the same path, or alternatively configuring
    /// the plot-specific archetypes through the blueprint.
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
    struct Scalar {
        /// The scalar value to log.
        rerun::components::Scalar scalar;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.ScalarIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        Scalar() = default;
        Scalar(Scalar&& other) = default;

        explicit Scalar(rerun::components::Scalar _scalar) : scalar(std::move(_scalar)) {}
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Scalar> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::Scalar& archetype);
    };
} // namespace rerun
