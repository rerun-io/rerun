// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/points3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/keypoint_id.hpp"
#include "../components/position3d.hpp"
#include "../components/radius.hpp"
#include "../components/show_labels.hpp"
#include "../components/text.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A 3D point cloud with positions and optional colors, radii, labels, etc.
    ///
    /// ## Examples
    ///
    /// ### Simple 3D points
    /// ![image](https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log("points", rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
    /// }
    /// ```
    ///
    /// ### Update a point cloud over time
    /// ![image](https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm>
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d_row_updates");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    ///     std::vector<std::array<float, 3>> positions[] = {
    ///         // clang-format off
    ///         {{1.0, 0.0, 1.0}, {0.5, 0.5, 2.0}},
    ///         {{1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0}},
    ///         {{2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5}},
    ///         {{-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5}},
    ///         {{1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0}},
    ///         // clang-format on
    ///     };
    ///
    ///     // At each timestep, all points in the cloud share the same but changing color and radius.
    ///     std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
    ///     std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};
    ///
    ///     for (size_t i = 0; i <5; i++) {
    ///         rec.set_time_seconds("time", 10.0 + static_cast<double>(i));
    ///         rec.log(
    ///             "points",
    ///             rerun::Points3D(positions[i]).with_colors(colors[i]).with_radii(radii[i])
    ///         );
    ///     }
    /// }
    /// ```
    ///
    /// ### Update a point cloud over time, in a single operation
    /// ![image](https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/full.png)
    ///
    /// ```cpp
    /// #include <array>
    /// #include <rerun.hpp>
    /// #include <vector>
    ///
    /// using namespace std::chrono_literals;
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d_column_updates");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    ///     std::vector<std::array<float, 3>> positions = {
    ///         // clang-format off
    ///         {1.0, 0.0, 1.0}, {0.5, 0.5, 2.0},
    ///         {1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0},
    ///         {2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5},
    ///         {-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5},
    ///         {1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0},
    ///         // clang-format on
    ///     };
    ///
    ///     // At each timestep, all points in the cloud share the same but changing color and radius.
    ///     std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
    ///     std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};
    ///
    ///     // Log at seconds 10-14
    ///     auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
    ///     auto time_column = rerun::TimeColumn::from_times("time", std::move(times));
    ///
    ///     // Partition our data as expected across the 5 timesteps.
    ///     auto position = rerun::Points3D().with_positions(positions).columns({2, 4, 4, 3, 4});
    ///     auto color_and_radius = rerun::Points3D().with_colors(colors).with_radii(radii).columns();
    ///
    ///     rec.send_columns("points", time_column, position, color_and_radius);
    /// }
    /// ```
    ///
    /// ### Update specific properties of a point cloud over time
    /// ![image](https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm>
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d_partial_updates");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     std::vector<rerun::Position3D> positions;
    ///     for (int i = 0; i <10; ++i) {
    ///         positions.emplace_back(static_cast<float>(i), 0.0f, 0.0f);
    ///     }
    ///
    ///     rec.set_time_sequence("frame", 0);
    ///     rec.log("points", rerun::Points3D(positions));
    ///
    ///     for (int i = 0; i <10; ++i) {
    ///         std::vector<rerun::Color> colors;
    ///         for (int n = 0; n <10; ++n) {
    ///             if (n <i) {
    ///                 colors.emplace_back(rerun::Color(20, 200, 20));
    ///             } else {
    ///                 colors.emplace_back(rerun::Color(200, 20, 20));
    ///             }
    ///         }
    ///
    ///         std::vector<rerun::Radius> radii;
    ///         for (int n = 0; n <10; ++n) {
    ///             if (n <i) {
    ///                 radii.emplace_back(rerun::Radius(0.6f));
    ///             } else {
    ///                 radii.emplace_back(rerun::Radius(0.2f));
    ///             }
    ///         }
    ///
    ///         // Update only the colors and radii, leaving everything else as-is.
    ///         rec.set_time_sequence("frame", i);
    ///         rec.log("points", rerun::Points3D::update_fields().with_radii(radii).with_colors(colors));
    ///     }
    ///
    ///     std::vector<rerun::Radius> radii;
    ///     radii.emplace_back(0.3f);
    ///
    ///     // Update the positions and radii, and clear everything else in the process.
    ///     rec.set_time_sequence("frame", 20);
    ///     rec.log("points", rerun::Points3D::clear_fields().with_positions(positions).with_radii(radii));
    /// }
    /// ```
    struct Points3D {
        /// All the 3D positions at which the point cloud shows points.
        std::optional<ComponentBatch> positions;

        /// Optional radii for the points, effectively turning them into circles.
        std::optional<ComponentBatch> radii;

        /// Optional colors for the points.
        std::optional<ComponentBatch> colors;

        /// Optional text labels for the points.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        std::optional<ComponentBatch> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<ComponentBatch> show_labels;

        /// Optional class Ids for the points.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<ComponentBatch> class_ids;

        /// Optional keypoint IDs for the points, identifying them within a class.
        ///
        /// If keypoint IDs are passed in but no `components::ClassId`s were specified, the `components::ClassId` will
        /// default to 0.
        /// This is useful to identify points within a single classification (which is identified
        /// with `class_id`).
        /// E.g. the classification might be 'Person' and the keypoints refer to joints on a
        /// detected skeleton.
        std::optional<ComponentBatch> keypoint_ids;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Points3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Points3D";

        /// `ComponentDescriptor` for the `positions` field.
        static constexpr auto Descriptor_positions = ComponentDescriptor(
            ArchetypeName, "positions",
            Loggable<rerun::components::Position3D>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `radii` field.
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `colors` field.
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `labels` field.
        static constexpr auto Descriptor_labels = ComponentDescriptor(
            ArchetypeName, "labels", Loggable<rerun::components::Text>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `show_labels` field.
        static constexpr auto Descriptor_show_labels = ComponentDescriptor(
            ArchetypeName, "show_labels",
            Loggable<rerun::components::ShowLabels>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `class_ids` field.
        static constexpr auto Descriptor_class_ids = ComponentDescriptor(
            ArchetypeName, "class_ids",
            Loggable<rerun::components::ClassId>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `keypoint_ids` field.
        static constexpr auto Descriptor_keypoint_ids = ComponentDescriptor(
            ArchetypeName, "keypoint_ids",
            Loggable<rerun::components::KeypointId>::Descriptor.component_name
        );

      public:
        Points3D() = default;
        Points3D(Points3D&& other) = default;
        Points3D(const Points3D& other) = default;
        Points3D& operator=(const Points3D& other) = default;
        Points3D& operator=(Points3D&& other) = default;

        explicit Points3D(Collection<rerun::components::Position3D> _positions)
            : positions(ComponentBatch::from_loggable(std::move(_positions), Descriptor_positions)
                            .value_or_throw()) {}

        /// Update only some specific fields of a `Points3D`.
        static Points3D update_fields() {
            return Points3D();
        }

        /// Clear all the fields of a `Points3D`.
        static Points3D clear_fields();

        /// All the 3D positions at which the point cloud shows points.
        Points3D with_positions(const Collection<rerun::components::Position3D>& _positions) && {
            positions =
                ComponentBatch::from_loggable(_positions, Descriptor_positions).value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for the points, effectively turning them into circles.
        Points3D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the points.
        Points3D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the points.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        Points3D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        Points3D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `show_labels` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_show_labels` should
        /// be used when logging a single row's worth of data.
        Points3D with_many_show_labels(const Collection<rerun::components::ShowLabels>& _show_labels
        ) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional class Ids for the points.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        Points3D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
            class_ids =
                ComponentBatch::from_loggable(_class_ids, Descriptor_class_ids).value_or_throw();
            return std::move(*this);
        }

        /// Optional keypoint IDs for the points, identifying them within a class.
        ///
        /// If keypoint IDs are passed in but no `components::ClassId`s were specified, the `components::ClassId` will
        /// default to 0.
        /// This is useful to identify points within a single classification (which is identified
        /// with `class_id`).
        /// E.g. the classification might be 'Person' and the keypoints refer to joints on a
        /// detected skeleton.
        Points3D with_keypoint_ids(const Collection<rerun::components::KeypointId>& _keypoint_ids
        ) && {
            keypoint_ids = ComponentBatch::from_loggable(_keypoint_ids, Descriptor_keypoint_ids)
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
    struct AsComponents<archetypes::Points3D> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(const archetypes::Points3D& archetype);
    };
} // namespace rerun
