// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/draw_order.hpp"
#include "../components/line_strip2d.hpp"
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
    /// **Archetype**: 2D line strips with positions and optional colors, radii, labels, etc.
    ///
    /// ## Examples
    ///
    /// ### line_strips2d_batch:
    /// ![image](https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_line_strip2d_batch");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rerun::Collection<rerun::Vec2D> strip1 = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}};
    ///     rerun::Collection<rerun::Vec2D> strip2 =
    ///         {{0.f, 3.f}, {1.f, 4.f}, {2.f, 2.f}, {3.f, 4.f}, {4.f, 2.f}, {5.f, 4.f}, {6.f, 3.f}};
    ///     rec.log(
    ///         "strips",
    ///         rerun::LineStrips2D({strip1, strip2})
    ///             .with_colors({0xFF0000FF, 0x00FF00FF})
    ///             .with_radii({0.025f, 0.005f})
    ///             .with_labels({"one strip here", "and one strip there"})
    ///     );
    ///
    ///     // TODO(#5520): log VisualBounds2D
    /// }
    /// ```
    ///
    /// ### Lines with scene & UI radius each
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_line_strip2d_ui_radius");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // A blue line with a scene unit radii of 0.01.
    ///     rerun::LineStrip2D linestrip_blue({{0.f, 0.f}, {0.f, 1.f}, {1.f, 0.f}, {1.f, 1.f}});
    ///     rec.log(
    ///         "scene_unit_line",
    ///         rerun::LineStrips2D(linestrip_blue)
    ///             // By default, radii are interpreted as world-space units.
    ///             .with_radii(0.01f)
    ///             .with_colors(rerun::Color(0, 0, 255))
    ///     );
    ///
    ///     // A red line with a ui point radii of 5.
    ///     // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    ///     // For 100 % ui scaling, UI points are equal to pixels.
    ///     rerun::LineStrip2D linestrip_red({{3.f, 0.f}, {3.f, 1.f}, {4.f, 0.f}, {4.f, 1.f}});
    ///     rec.log(
    ///         "ui_points_line",
    ///         rerun::LineStrips2D(linestrip_red)
    ///             // By default, radii are interpreted as world-space units.
    ///             .with_radii(rerun::Radius::ui_points(5.0f))
    ///             .with_colors(rerun::Color(255, 0, 0))
    ///     );
    ///
    ///     // TODO(#5520): log VisualBounds2D
    /// }
    /// ```
    struct LineStrips2D {
        /// All the actual 2D line strips that make up the batch.
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

        /// An optional floating point value that specifies the 2D drawing order of each line strip.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<ComponentBatch> draw_order;

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<ComponentBatch> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.LineStrips2DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.LineStrips2D";

        /// `ComponentDescriptor` for the `strips` field.
        static constexpr auto Descriptor_strips = ComponentDescriptor(
            ArchetypeName, "strips",
            Loggable<rerun::components::LineStrip2D>::Descriptor.component_name
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
        /// `ComponentDescriptor` for the `draw_order` field.
        static constexpr auto Descriptor_draw_order = ComponentDescriptor(
            ArchetypeName, "draw_order",
            Loggable<rerun::components::DrawOrder>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `class_ids` field.
        static constexpr auto Descriptor_class_ids = ComponentDescriptor(
            ArchetypeName, "class_ids",
            Loggable<rerun::components::ClassId>::Descriptor.component_name
        );

      public:
        LineStrips2D() = default;
        LineStrips2D(LineStrips2D&& other) = default;
        LineStrips2D(const LineStrips2D& other) = default;
        LineStrips2D& operator=(const LineStrips2D& other) = default;
        LineStrips2D& operator=(LineStrips2D&& other) = default;

        explicit LineStrips2D(Collection<rerun::components::LineStrip2D> _strips)
            : strips(ComponentBatch::from_loggable(std::move(_strips), Descriptor_strips)
                         .value_or_throw()) {}

        /// Update only some specific fields of a `LineStrips2D`.
        static LineStrips2D update_fields() {
            return LineStrips2D();
        }

        /// Clear all the fields of a `LineStrips2D`.
        static LineStrips2D clear_fields();

        /// All the actual 2D line strips that make up the batch.
        LineStrips2D with_strips(const Collection<rerun::components::LineStrip2D>& _strips) && {
            strips = ComponentBatch::from_loggable(_strips, Descriptor_strips).value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for the line strips.
        LineStrips2D with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the line strips.
        LineStrips2D with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the line strips.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        LineStrips2D with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        LineStrips2D with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `rerun:: components:: ShowLabels in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_show_labels` should
        /// be used when logging a single row's worth of data.
        LineStrips2D with_many_show_labels(
            const Collection<rerun::components::ShowLabels>& _show_labels
        ) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// An optional floating point value that specifies the 2D drawing order of each line strip.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        LineStrips2D with_draw_order(const rerun::components::DrawOrder& _draw_order) && {
            draw_order =
                ComponentBatch::from_loggable(_draw_order, Descriptor_draw_order).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `rerun:: components:: DrawOrder in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_draw_order` should
        /// be used when logging a single row's worth of data.
        LineStrips2D with_many_draw_order(
            const Collection<rerun::components::DrawOrder>& _draw_order
        ) && {
            draw_order =
                ComponentBatch::from_loggable(_draw_order, Descriptor_draw_order).value_or_throw();
            return std::move(*this);
        }

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        LineStrips2D with_class_ids(const Collection<rerun::components::ClassId>& _class_ids) && {
            class_ids =
                ComponentBatch::from_loggable(_class_ids, Descriptor_class_ids).value_or_throw();
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
    struct AsComponents<archetypes::LineStrips2D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::LineStrips2D& archetype
        );
    };
} // namespace rerun
