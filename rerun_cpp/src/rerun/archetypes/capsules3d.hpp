// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/capsules3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/length.hpp"
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
    /// **Archetype**: 3D capsules; cylinders with hemispherical caps.
    ///
    /// Capsules are defined by two endpoints (the centers of their end cap spheres), which are located
    /// at (0, 0, 0) and (0, 0, length), that is, extending along the positive direction of the Z axis.
    /// Capsules in other orientations may be produced by applying a rotation to the entity or
    /// instances.
    ///
    /// ## Example
    ///
    /// ### Batch of capsules
    /// ![image](https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_capsule3d_batch");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log(
    ///         "capsules",
    ///         rerun::Capsules3D::from_lengths_and_radii(
    ///             {0.0f, 2.0f, 4.0f, 6.0f, 8.0f},
    ///             {1.0f, 0.5f, 0.5f, 0.5f, 1.0f}
    ///         )
    ///             .with_colors({
    ///                 rerun::Rgba32(255, 0, 0),
    ///                 rerun::Rgba32(188, 188, 0),
    ///                 rerun::Rgba32(0, 255, 0),
    ///                 rerun::Rgba32(0, 188, 188),
    ///                 rerun::Rgba32(0, 0, 255),
    ///             })
    ///             .with_translations({
    ///                 {0.0f, 0.0f, 0.0f},
    ///                 {2.0f, 0.0f, 0.0f},
    ///                 {4.0f, 0.0f, 0.0f},
    ///                 {6.0f, 0.0f, 0.0f},
    ///                 {8.0f, 0.0f, 0.0f},
    ///             })
    ///             .with_rotation_axis_angles({
    ///                 rerun::RotationAxisAngle(),
    ///                 rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-22.5)),
    ///                 rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-45.0)),
    ///                 rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-67.5)),
    ///                 rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-90.0)),
    ///             })
    ///     );
    /// }
    /// ```
    struct Capsules3D {
        /// Lengths of the capsules, defined as the distance between the centers of the endcaps.
        std::optional<ComponentBatch> lengths;

        /// Radii of the capsules.
        std::optional<ComponentBatch> radii;

        /// Optional translations of the capsules.
        ///
        /// If not specified, one end of each capsule will be at (0, 0, 0).
        /// Note that this uses a `components::PoseTranslation3D` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> translations;

        /// Rotations via axis + angle.
        ///
        /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
        /// Note that this uses a `components::PoseRotationAxisAngle` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> rotation_axis_angles;

        /// Rotations via quaternion.
        ///
        /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
        /// Note that this uses a `components::PoseRotationQuat` which is also used by `archetypes::InstancePoses3D`.
        std::optional<ComponentBatch> quaternions;

        /// Optional colors for the capsules.
        std::optional<ComponentBatch> colors;

        /// Optional text labels for the capsules, which will be located at their centers.
        std::optional<ComponentBatch> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<ComponentBatch> show_labels;

        /// Optional class ID for the ellipsoids.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        std::optional<ComponentBatch> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.Capsules3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Capsules3D";

        /// `ComponentDescriptor` for the `lengths` field.
        static constexpr auto Descriptor_lengths = ComponentDescriptor(
            ArchetypeName, "lengths", Loggable<rerun::components::Length>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `radii` field.
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `translations` field.
        static constexpr auto Descriptor_translations = ComponentDescriptor(
            ArchetypeName, "translations",
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

      public: // START of extensions from capsules3d_ext.cpp:
        /// Creates a new `Capsules3D` with the given axis-aligned lengths and radii.
        ///
        /// For multiple capsules, you should generally follow this with
        /// `Capsules3D::with_translations()` and one of the rotation methods, in order to move them
        /// apart from each other.
        //
        // TODO(andreas): This should not take an std::vector.
        static Capsules3D from_lengths_and_radii(
            const std::vector<float>& lengths, const std::vector<float>& radii
        ) {
            return Capsules3D().with_lengths(std::move(lengths)).with_radii(std::move(radii));
        }

        /* TODO(kpreid): This should exist for parity with Rust, but actually implementing this
           needs a bit of quaternion math.

        /// Creates a new `Capsules3D` where each capsule extends between the given pairs of points.
        //
        // TODO(andreas): This should not take an std::vector.
        //
        static Capsules3D from_endpoints_and_radii(
            const std::vector<datatypes::Vec3D>& start_points,
            const std::vector<datatypes::Vec3D>& end_points,
            const std::vector<float>& radii
        );
        */

        // END of extensions from capsules3d_ext.cpp, start of generated code:

      public:
        Capsules3D() = default;
        Capsules3D(Capsules3D&& other) = default;
        Capsules3D(const Capsules3D& other) = default;
        Capsules3D& operator=(const Capsules3D& other) = default;
        Capsules3D& operator=(Capsules3D&& other) = default;

        /// Update only some specific fields of a `Capsules3D`.
        static Capsules3D update_fields() {
            return Capsules3D();
        }

        /// Clear all the fields of a `Capsules3D`.
        static Capsules3D clear_fields();

        /// Lengths of the capsules, defined as the distance between the centers of the endcaps.
        Capsules3D with_lengths(const Collection<rerun::components::Length>& _lengths) && {
            lengths = ComponentBatch::from_loggable(_lengths, Descriptor_lengths).value_or_throw();
            return std::move(*this);
        }

        /// Radii of the capsules.
        Capsules3D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optional translations of the capsules.
        ///
        /// If not specified, one end of each capsule will be at (0, 0, 0).
        /// Note that this uses a `components::PoseTranslation3D` which is also used by `archetypes::InstancePoses3D`.
        Capsules3D with_translations(
            const Collection<rerun::components::PoseTranslation3D>& _translations
        ) && {
            translations = ComponentBatch::from_loggable(_translations, Descriptor_translations)
                               .value_or_throw();
            return std::move(*this);
        }

        /// Rotations via axis + angle.
        ///
        /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
        /// Note that this uses a `components::PoseRotationAxisAngle` which is also used by `archetypes::InstancePoses3D`.
        Capsules3D with_rotation_axis_angles(
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
        /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
        /// Note that this uses a `components::PoseRotationQuat` which is also used by `archetypes::InstancePoses3D`.
        Capsules3D with_quaternions(
            const Collection<rerun::components::PoseRotationQuat>& _quaternions
        ) && {
            quaternions = ComponentBatch::from_loggable(_quaternions, Descriptor_quaternions)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the capsules.
        Capsules3D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the capsules, which will be located at their centers.
        Capsules3D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        Capsules3D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `show_labels` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_show_labels` should
        /// be used when logging a single row's worth of data.
        Capsules3D with_many_show_labels(
            const Collection<rerun::components::ShowLabels>& _show_labels
        ) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional class ID for the ellipsoids.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        Capsules3D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
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
    struct AsComponents<archetypes::Capsules3D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Capsules3D& archetype
        );
    };
} // namespace rerun
