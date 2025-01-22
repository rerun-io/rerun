// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/scalar.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/scalar.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
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
    /// ## Examples
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
    ///
    /// ### Multiple scalars in a single `send_columns` call
    /// ![image](https://static.rerun.io/scalar_send_columns/b4bf172256f521f4851dfec5c2c6e3143f5d6923/full.png)
    ///
    /// ```cpp
    /// #include <cmath>
    /// #include <numeric>
    /// #include <vector>
    ///
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_scalar_send_columns");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Native scalars & times.
    ///     std::vector<double> scalar_data(64);
    ///     for (size_t i = 0; i <64; ++i) {
    ///         scalar_data[i] = sin(static_cast<double>(i) / 10.0);
    ///     }
    ///     std::vector<int64_t> times(64);
    ///     std::iota(times.begin(), times.end(), 0);
    ///
    ///     // Convert to rerun time / scalars
    ///     auto time_column = rerun::TimeColumn::from_sequence_points("step", std::move(times));
    ///     auto scalar_data_collection =
    ///         rerun::Collection<rerun::components::Scalar>(std::move(scalar_data));
    ///
    ///     rec.send_columns("scalars", time_column, scalar_data_collection);
    /// }
    /// ```
    struct Scalar {
        /// The scalar value to log.
        std::optional<ComponentBatch> scalar;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.ScalarIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Scalar";

        /// `ComponentDescriptor` for the `scalar` field.
        static constexpr auto Descriptor_scalar = ComponentDescriptor(
            ArchetypeName, "scalar", Loggable<rerun::components::Scalar>::Descriptor.component_name
        );

      public:
        Scalar() = default;
        Scalar(Scalar&& other) = default;
        Scalar(const Scalar& other) = default;
        Scalar& operator=(const Scalar& other) = default;
        Scalar& operator=(Scalar&& other) = default;

        explicit Scalar(rerun::components::Scalar _scalar)
            : scalar(ComponentBatch::from_loggable(std::move(_scalar), Descriptor_scalar)
                         .value_or_throw()) {}

        /// Update only some specific fields of a `Scalar`.
        static Scalar update_fields() {
            return Scalar();
        }

        /// Clear all the fields of a `Scalar`.
        static Scalar clear_fields();

        /// The scalar value to log.
        Scalar with_scalar(const rerun::components::Scalar& _scalar) && {
            scalar = ComponentBatch::from_loggable(_scalar, Descriptor_scalar).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
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
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Scalar& archetype);
    };
} // namespace rerun
