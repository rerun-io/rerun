// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/line_strips3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/line_strip3d.hpp"
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
    /// **Archetype**: 3D line strips with positions and optional colors, radii, labels, etc.
    ///
    /// ## Examples
    ///
    /// ### Many strips
    /// ![image](https://static.rerun.io/line_strip3d_batch/15e8ff18a6c95a3191acb0eae6eb04adea3b4874/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_line_strip3d_batch");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rerun::Collection<rerun::Vec3D> strip1 = {
    ///         {0.f, 0.f, 2.f},
    ///         {1.f, 0.f, 2.f},
    ///         {1.f, 1.f, 2.f},
    ///         {0.f, 1.f, 2.f},
    ///     };
    ///     rerun::Collection<rerun::Vec3D> strip2 = {
    ///         {0.f, 0.f, 0.f},
    ///         {0.f, 0.f, 1.f},
    ///         {1.f, 0.f, 0.f},
    ///         {1.f, 0.f, 1.f},
    ///         {1.f, 1.f, 0.f},
    ///         {1.f, 1.f, 1.f},
    ///         {0.f, 1.f, 0.f},
    ///         {0.f, 1.f, 1.f},
    ///     };
    ///     rec.log(
    ///         "strips",
    ///         rerun::LineStrips3D({strip1, strip2})
    ///             .with_colors({0xFF0000FF, 0x00FF00FF})
    ///             .with_radii({0.025f, 0.005f})
    ///             .with_labels({"one strip here", "and one strip there"})
    ///     );
    /// }
    /// ```
    ///
    /// ### Lines with scene & UI radius each
    /// ![image](https://static.rerun.io/line_strip3d_ui_radius/36b98f47e45747b5a3601511ff39b8d74c61d120/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_line_strip3d_ui_radius");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // A blue line with a scene unit radii of 0.01.
    ///     rerun::LineStrip3D linestrip_blue(
    ///         {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}, {1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}}
    ///     );
    ///     rec.log(
    ///         "scene_unit_line",
    ///         rerun::LineStrips3D(linestrip_blue)
    ///             // By default, radii are interpreted as world-space units.
    ///             .with_radii(0.01f)
    ///             .with_colors(rerun::Color(0, 0, 255))
    ///     );
    ///
    ///     // A red line with a ui point radii of 5.
    ///     // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    ///     // For 100 % ui scaling, UI points are equal to pixels.
    ///     rerun::LineStrip3D linestrip_red(
    ///         {{3.f, 0.f, 0.f}, {3.f, 0.f, 1.f}, {4.f, 0.f, 0.f}, {4.f, 0.f, 1.f}}
    ///     );
    ///     rec.log(
    ///         "ui_points_line",
    ///         rerun::LineStrips3D(linestrip_red)
    ///             // By default, radii are interpreted as world-space units.
    ///             .with_radii(rerun::Radius::ui_points(5.0f))
    ///             .with_colors(rerun::Color(255, 0, 0))
    ///     );
    /// }
    /// ```
    struct LineStrips3D {
        /// All the actual 3D line strips that make up the batch.
        std::optional<ComponentBatch> strips;

        /// Optional radii for the line strips.
        std::optional<ComponentBatch> radii;

        /// Optional colors for the line strips.
        std::optional<ComponentBatch> colors;

        /// Optional text labels for the line strips.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        std::optional<ComponentBatch> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<ComponentBatch> show_labels;

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<ComponentBatch> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.LineStrips3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.LineStrips3D";

        /// `ComponentDescriptor` for the `strips` field.
        static constexpr auto Descriptor_strips = ComponentDescriptor(
            ArchetypeName, "strips",
            Loggable<rerun::components::LineStrip3D>::Descriptor.component_name
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

      public:
        LineStrips3D() = default;
        LineStrips3D(LineStrips3D&& other) = default;
        LineStrips3D(const LineStrips3D& other) = default;
        LineStrips3D& operator=(const LineStrips3D& other) = default;
        LineStrips3D& operator=(LineStrips3D&& other) = default;

        explicit LineStrips3D(Collection<rerun::components::LineStrip3D> _strips)
            : strips(ComponentBatch::from_loggable(std::move(_strips), Descriptor_strips)
                         .value_or_throw()) {}

        /// Update only some specific fields of a `LineStrips3D`.
        static LineStrips3D update_fields() {
            return LineStrips3D();
        }

        /// Clear all the fields of a `LineStrips3D`.
        static LineStrips3D clear_fields();

        /// All the actual 3D line strips that make up the batch.
        LineStrips3D with_strips(const Collection<rerun::components::LineStrip3D>& _strips) && {
            strips = ComponentBatch::from_loggable(_strips, Descriptor_strips).value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for the line strips.
        LineStrips3D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the line strips.
        LineStrips3D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the line strips.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        LineStrips3D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        LineStrips3D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        LineStrips3D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
            class_ids =
                ComponentBatch::from_loggable(_class_ids, Descriptor_class_ids).value_or_throw();
            return std::move(*this);
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::LineStrips3D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::LineStrips3D& archetype
        );
    };
} // namespace rerun
