// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/draw_order.hpp"
#include "../components/line_strip2d.hpp"
#include "../components/radius.hpp"
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
    /// ### line_strip2d_batch:
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
    struct LineStrips2D {
        /// All the actual 2D line strips that make up the batch.
        Collection<rerun::components::LineStrip2D> strips;

        /// Optional radii for the line strips.
        std::optional<Collection<rerun::components::Radius>> radii;

        /// Optional colors for the line strips.
        std::optional<Collection<rerun::components::Color>> colors;

        /// Optional text labels for the line strips.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        std::optional<Collection<rerun::components::Text>> labels;

        /// An optional floating point value that specifies the 2D drawing order of each line strip.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<rerun::components::DrawOrder> draw_order;

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<Collection<rerun::components::ClassId>> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.LineStrips2DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        LineStrips2D() = default;
        LineStrips2D(LineStrips2D&& other) = default;

        explicit LineStrips2D(Collection<rerun::components::LineStrip2D> _strips)
            : strips(std::move(_strips)) {}

        /// Optional radii for the line strips.
        LineStrips2D with_radii(Collection<rerun::components::Radius> _radii) && {
            radii = std::move(_radii);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional colors for the line strips.
        LineStrips2D with_colors(Collection<rerun::components::Color> _colors) && {
            colors = std::move(_colors);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional text labels for the line strips.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        LineStrips2D with_labels(Collection<rerun::components::Text> _labels) && {
            labels = std::move(_labels);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional floating point value that specifies the 2D drawing order of each line strip.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        LineStrips2D with_draw_order(rerun::components::DrawOrder _draw_order) && {
            draw_order = std::move(_draw_order);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional `components::ClassId`s for the lines.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        LineStrips2D with_class_ids(Collection<rerun::components::ClassId> _class_ids) && {
            class_ids = std::move(_class_ids);
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
    struct AsComponents<archetypes::LineStrips2D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::LineStrips2D& archetype
        );
    };
} // namespace rerun
