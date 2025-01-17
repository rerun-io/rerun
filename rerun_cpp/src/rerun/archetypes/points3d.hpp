// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/points3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
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
    /// ### Randomly distributed 3D points with varying color and radius
    /// ![image](https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm>
    /// #include <random>
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d_random");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     std::default_random_engine gen;
    ///     std::uniform_real_distribution<float> dist_pos(-5.0f, 5.0f);
    ///     std::uniform_real_distribution<float> dist_radius(0.1f, 1.0f);
    ///     // On MSVC uint8_t distributions are not supported.
    ///     std::uniform_int_distribution<int> dist_color(0, 255);
    ///
    ///     std::vector<rerun::Position3D> points3d(10);
    ///     std::generate(points3d.begin(), points3d.end(), [&] {
    ///         return rerun::Position3D(dist_pos(gen), dist_pos(gen), dist_pos(gen));
    ///     });
    ///     std::vector<rerun::Color> colors(10);
    ///     std::generate(colors.begin(), colors.end(), [&] {
    ///         return rerun::Color(
    ///             static_cast<uint8_t>(dist_color(gen)),
    ///             static_cast<uint8_t>(dist_color(gen)),
    ///             static_cast<uint8_t>(dist_color(gen))
    ///         );
    ///     });
    ///     std::vector<rerun::Radius> radii(10);
    ///     std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });
    ///
    ///     rec.log("random", rerun::Points3D(points3d).with_colors(colors).with_radii(radii));
    /// }
    /// ```
    ///
    /// ### Log points with radii given in UI points
    /// ![image](https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_points3d_ui_radius");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Two blue points with scene unit radii of 0.1 and 0.3.
    ///     rec.log(
    ///         "scene_units",
    ///         rerun::Points3D({{0.0f, 1.0f, 0.0f}, {1.0f, 1.0f, 1.0f}})
    ///             // By default, radii are interpreted as world-space units.
    ///             .with_radii({0.1f, 0.3f})
    ///             .with_colors(rerun::Color(0, 0, 255))
    ///     );
    ///
    ///     // Two red points with ui point radii of 40 and 60.
    ///     // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    ///     // For 100% ui scaling, UI points are equal to pixels.
    ///     rec.log(
    ///         "ui_points",
    ///         rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 0.0f, 1.0f}})
    ///             // rerun::Radius::ui_points produces radii that the viewer interprets as given in ui points.
    ///             .with_radii({
    ///                 rerun::Radius::ui_points(40.0f),
    ///                 rerun::Radius::ui_points(60.0f),
    ///             })
    ///             .with_colors(rerun::Color(255, 0, 0))
    ///     );
    /// }
    /// ```
    ///
    /// ### Send several point clouds with varying point count over time in a single call
    /// ![image](https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/full.png)
    ///
    /// ```cpp
    /// #include <array>
    /// #include <rerun.hpp>
    /// #include <vector>
    ///
    /// using namespace std::chrono_literals;
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_send_columns_arrays");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Prepare a point cloud that evolves over time 5 timesteps, changing the number of points in the process.
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
    ///     // At each time stamp, all points in the cloud share the same but changing color.
    ///     std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
    ///
    ///     // Log at seconds 10-14
    ///     auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
    ///     auto time_column = rerun::TimeColumn::from_times("time", std::move(times));
    ///
    ///     // Interpret raw positions and color data as rerun components and partition them.
    ///     auto indicator_batch = rerun::ComponentColumn::from_indicators<rerun::Points3D>(5);
    ///     auto position_batch = rerun::ComponentColumn::from_loggable_with_lengths(
    ///         rerun::Collection<rerun::components::Position3D>(std::move(positions)),
    ///         {2, 4, 4, 3, 4}
    ///     );
    ///     auto color_batch = rerun::ComponentColumn::from_loggable(
    ///         rerun::Collection<rerun::components::Color>(std::move(colors))
    ///     );
    ///
    ///     rec.send_columns(
    ///         "points",
    ///         time_column,
    ///         {
    ///             indicator_batch.value_or_throw(),
    ///             position_batch.value_or_throw(),
    ///             color_batch.value_or_throw(),
    ///         }
    ///     );
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
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Points3D";
        static constexpr auto Descriptor_positions = ComponentDescriptor(
            ArchetypeName, "positions",
            Loggable<rerun::components::Position3D>::Descriptor.component_name
        );
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        static constexpr auto Descriptor_labels = ComponentDescriptor(
            ArchetypeName, "labels", Loggable<rerun::components::Text>::Descriptor.component_name
        );
        static constexpr auto Descriptor_show_labels = ComponentDescriptor(
            ArchetypeName, "show_labels",
            Loggable<rerun::components::ShowLabels>::Descriptor.component_name
        );
        static constexpr auto Descriptor_class_ids = ComponentDescriptor(
            ArchetypeName, "class_ids",
            Loggable<rerun::components::ClassId>::Descriptor.component_name
        );
        static constexpr auto Descriptor_keypoint_ids = ComponentDescriptor(
            ArchetypeName, "keypoint_ids",
            Loggable<rerun::components::KeypointId>::Descriptor.component_name
        );

      public:
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
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional radii for the points, effectively turning them into circles.
        Points3D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional colors for the points.
        Points3D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional text labels for the points.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        Points3D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional choice of whether the text labels should be shown by default.
        Points3D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional class Ids for the points.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        Points3D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
            class_ids =
                ComponentBatch::from_loggable(_class_ids, Descriptor_class_ids).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
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
    struct AsComponents<archetypes::Points3D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Points3D& archetype);
    };
} // namespace rerun
