// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/scalar.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
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
    /// ### Update a scalar over time
    /// ![image](https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/full.png)
    ///
    /// ```cpp
    /// #include <cmath>
    ///
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_scalar_row_updates");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     for (int step = 0; step <64; ++step) {
    ///         rec.set_index("step", sequence=step);
    ///         rec.log("scalars", rerun::Scalar(sin(static_cast<double>(step) / 10.0)));
    ///     }
    /// }
    /// ```
    ///
    /// ### Update a scalar over time, in a single operation
    /// ![image](https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/full.png)
    ///
    /// ```cpp
    /// #include <cmath>
    /// #include <numeric>
    /// #include <vector>
    ///
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_scalar_column_updates");
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
    ///     // Serialize to columns and send.
    ///     rec.send_columns(
    ///         "scalars",
    ///         rerun::TimeColumn::from_sequence("step", std::move(times)),
    ///         rerun::Scalar().with_many_scalar(std::move(scalar_data)).columns()
    ///     );
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
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `scalar` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_scalar` should
        /// be used when logging a single row's worth of data.
        Scalar with_many_scalar(const Collection<rerun::components::Scalar>& _scalar) && {
            scalar = ComponentBatch::from_loggable(_scalar, Descriptor_scalar).value_or_throw();
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
    struct AsComponents<archetypes::Scalar> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(const archetypes::Scalar& archetype);
    };
} // namespace rerun
