// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/boxes3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/fill_mode.hpp"
#include "../components/half_size3d.hpp"
#include "../components/pose_rotation_axis_angle.hpp"
#include "../components/pose_rotation_quat.hpp"
#include "../components/pose_translation3d.hpp"
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
    /// **Archetype**: 3D boxes with half-extents and optional center, rotations, colors etc.
    ///
    /// Note that orienting and placing the box is handled via `[archetypes.InstancePoses3D]`.
    /// Some of its component are repeated here for convenience.
    /// If there's more instance poses than half sizes, the last half size will be repeated for the remaining poses.
    ///
    /// ## Example
    ///
    /// ### Batch of 3D boxes
    /// ![image](https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_box3d_batch");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log(
    ///         "batch",
    ///         rerun::Boxes3D::from_centers_and_half_sizes(
    ///             {{2.0f, 0.0f, 0.0f}, {-2.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 2.0f}},
    ///             {{2.0f, 2.0f, 1.0f}, {1.0f, 1.0f, 0.5f}, {2.0f, 0.5f, 1.0f}}
    ///         )
    ///             .with_quaternions({
    ///                 rerun::Quaternion::IDENTITY,
    ///                 // 45 degrees around Z
    ///                 rerun::Quaternion::from_xyzw(0.0f, 0.0f, 0.382683f, 0.923880f),
    ///             })
    ///             .with_radii({0.025f})
    ///             .with_colors({
    ///                 rerun::Rgba32(255, 0, 0),
    ///                 rerun::Rgba32(0, 255, 0),
    ///                 rerun::Rgba32(0, 0, 255),
    ///             })
    ///             .with_fill_mode(rerun::FillMode::Solid)
    ///             .with_labels({"red", "green", "blue"})
    ///     );
    /// }
    /// ```
    struct Boxes3D {
        /// All half-extents that make up the batch of boxes.
        std::optional<ComponentBatch> half_sizes;

        /// Optional center positions of the boxes.
        ///
        /// If not specified, the centers will be at (0, 0, 0).
        /// Note that this uses a `components::PoseTranslation3D` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> centers;

        /// Rotations via axis + angle.
        ///
        /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
        /// Note that this uses a `components::PoseRotationAxisAngle` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> rotation_axis_angles;

        /// Rotations via quaternion.
        ///
        /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
        /// Note that this uses a `components::PoseRotationQuat` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> quaternions;

        /// Optional colors for the boxes.
        std::optional<ComponentBatch> colors;

        /// Optional radii for the lines that make up the boxes.
        std::optional<ComponentBatch> radii;

        /// Optionally choose whether the boxes are drawn with lines or solid.
        std::optional<ComponentBatch> fill_mode;

        /// Optional text labels for the boxes.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        std::optional<ComponentBatch> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<ComponentBatch> show_labels;

        /// Optional `components::ClassId`s for the boxes.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<ComponentBatch> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Boxes3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Boxes3D";

        /// `ComponentDescriptor` for the `half_sizes` field.
        static constexpr auto Descriptor_half_sizes = ComponentDescriptor(
            ArchetypeName, "half_sizes",
            Loggable<rerun::components::HalfSize3D>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `centers` field.
        static constexpr auto Descriptor_centers = ComponentDescriptor(
            ArchetypeName, "centers",
            Loggable<rerun::components::PoseTranslation3D>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `rotation_axis_angles` field.
        static constexpr auto Descriptor_rotation_axis_angles = ComponentDescriptor(
            ArchetypeName, "rotation_axis_angles",
            Loggable<rerun::components::PoseRotationAxisAngle>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `quaternions` field.
        static constexpr auto Descriptor_quaternions = ComponentDescriptor(
            ArchetypeName, "quaternions",
            Loggable<rerun::components::PoseRotationQuat>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `colors` field.
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `radii` field.
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `fill_mode` field.
        static constexpr auto Descriptor_fill_mode = ComponentDescriptor(
            ArchetypeName, "fill_mode",
            Loggable<rerun::components::FillMode>::Descriptor.component_name
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

      public: // START of extensions from boxes3d_ext.cpp:
        /// Creates new `Boxes3D` with `half_sizes` centered around the local origin.
        static Boxes3D from_half_sizes(Collection<components::HalfSize3D> half_sizes) {
            return Boxes3D().with_half_sizes(std::move(half_sizes));
        }

        /// Creates new `Boxes3D` with `centers` and `half_sizes`.
        static Boxes3D from_centers_and_half_sizes(
            Collection<components::PoseTranslation3D> centers,
            Collection<components::HalfSize3D> half_sizes
        ) {
            return Boxes3D()
                .with_half_sizes(std::move(half_sizes))
                .with_centers(std::move(centers));
        }

        /// Creates new `Boxes3D` with `half_sizes` created from (full) sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the
        /// input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_sizes(const std::vector<datatypes::Vec3D>& sizes);

        /// Creates new `Boxes3D` with `centers` and `half_sizes` created from centers and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_centers_and_sizes(
            Collection<components::PoseTranslation3D> centers,
            const std::vector<datatypes::Vec3D>& sizes
        ) {
            return from_sizes(std::move(sizes)).with_centers(std::move(centers));
        }

        /// Creates new `Boxes3D` with `half_sizes` and `centers` created from minimums and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_mins_and_sizes(
            const std::vector<datatypes::Vec3D>& mins, const std::vector<datatypes::Vec3D>& sizes
        );

        // END of extensions from boxes3d_ext.cpp, start of generated code:

      public:
        Boxes3D() = default;
        Boxes3D(Boxes3D&& other) = default;
        Boxes3D(const Boxes3D& other) = default;
        Boxes3D& operator=(const Boxes3D& other) = default;
        Boxes3D& operator=(Boxes3D&& other) = default;

        /// Update only some specific fields of a `Boxes3D`.
        static Boxes3D update_fields() {
            return Boxes3D();
        }

        /// Clear all the fields of a `Boxes3D`.
        static Boxes3D clear_fields();

        /// All half-extents that make up the batch of boxes.
        Boxes3D with_half_sizes(const Collection<rerun::components::HalfSize3D>& _half_sizes) && {
            half_sizes =
                ComponentBatch::from_loggable(_half_sizes, Descriptor_half_sizes).value_or_throw();
            return std::move(*this);
        }

        /// Optional center positions of the boxes.
        ///
        /// If not specified, the centers will be at (0, 0, 0).
        /// Note that this uses a `components::PoseTranslation3D` which is also used by `archetypes::InstancePoses3D`.
        Boxes3D with_centers(const Collection<rerun::components::PoseTranslation3D>& _centers) && {
            centers = ComponentBatch::from_loggable(_centers, Descriptor_centers).value_or_throw();
            return std::move(*this);
        }

        /// Rotations via axis + angle.
        ///
        /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
        /// Note that this uses a `components::PoseRotationAxisAngle` which is also used by `archetypes::InstancePoses3D`.
        Boxes3D with_rotation_axis_angles(
            const Collection<rerun::components::PoseRotationAxisAngle>& _rotation_axis_angles
        ) && {
            rotation_axis_angles = ComponentBatch::from_loggable(
                                       _rotation_axis_angles,
                                       Descriptor_rotation_axis_angles
            )
                                       .value_or_throw();
            return std::move(*this);
        }

        /// Rotations via quaternion.
        ///
        /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
        /// Note that this uses a `components::PoseRotationQuat` which is also used by `archetypes::InstancePoses3D`.
        Boxes3D with_quaternions(const Collection<rerun::components::PoseRotationQuat>& _quaternions
        ) && {
            quaternions = ComponentBatch::from_loggable(_quaternions, Descriptor_quaternions)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the boxes.
        Boxes3D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for the lines that make up the boxes.
        Boxes3D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optionally choose whether the boxes are drawn with lines or solid.
        Boxes3D with_fill_mode(const rerun::components::FillMode& _fill_mode) && {
            fill_mode =
                ComponentBatch::from_loggable(_fill_mode, Descriptor_fill_mode).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `fill_mode` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_fill_mode` should
        /// be used when logging a single row's worth of data.
        Boxes3D with_many_fill_mode(const Collection<rerun::components::FillMode>& _fill_mode) && {
            fill_mode =
                ComponentBatch::from_loggable(_fill_mode, Descriptor_fill_mode).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the boxes.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        Boxes3D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        Boxes3D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `show_labels` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_show_labels` should
        /// be used when logging a single row's worth of data.
        Boxes3D with_many_show_labels(const Collection<rerun::components::ShowLabels>& _show_labels
        ) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional `components::ClassId`s for the boxes.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        Boxes3D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
            class_ids =
                ComponentBatch::from_loggable(_class_ids, Descriptor_class_ids).value_or_throw();
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
    struct AsComponents<archetypes::Boxes3D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Boxes3D& archetype);
    };
} // namespace rerun
