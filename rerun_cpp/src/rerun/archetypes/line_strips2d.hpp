// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/draw_order.hpp"
#include "../components/instance_key.hpp"
#include "../components/line_strip2d.hpp"
#include "../components/radius.hpp"
#include "../components/text.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// A batch of line strips with positions and optional colors, radii, labels, etc.
        ///
        /// ## Examples
        ///
        /// Many strips:
        /// ```cpp,ignore
        /// // Log a batch of 2d line strips.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///     auto rec = rr::RecordingStream("rerun_example_line_strip2d");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     std::vector<rr::datatypes::Vec2D> strip1 = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f},
        ///     {6.f, 0.f}}; std::vector<rr::datatypes::Vec2D> strip2 =
        ///         {{0.f, 3.f}, {1.f, 4.f}, {2.f, 2.f}, {3.f, 4.f}, {4.f, 2.f}, {5.f, 4.f},
        ///         {6.f, 3.f}};
        ///     rec.log(
        ///         "strips",
        ///         rr::LineStrips2D({strip1, strip2})
        ///             .with_colors({0xFF0000FF, 0x00FF00FF})
        ///             .with_radii({0.025f, 0.005f})
        ///             .with_labels({"one strip here", "and one strip there"})
        ///     );
        ///
        ///     // Log an extra rect to set the view bounds
        ///     rec.log("bounds", rr::Boxes2D::from_centers_and_sizes({{3.0f, 1.5f}},
        ///     {{8.0f, 9.0f}}));
        /// }
        /// ```
        ///
        /// Many individual segments:
        /// ```cpp,ignore
        /// // Log a couple 2D line segments using 2D line strips.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///     auto rec = rr::RecordingStream("rerun_example_line_segments2d");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     // TODO(#3202): I want to do this!
        ///     // std::vector<std::vector<rr::datatypes::Vec2D>> points = {
        ///     //     {{0.f, 0.f}, {2.f, 1.f}},
        ///     //     {{4.f, -1.f}, {6.f, 0.f}},
        ///     // };
        ///     // rec.log("segments", rr::LineStrips2D(points));
        ///
        ///     std::vector<rr::datatypes::Vec2D> points1 = {{0.f, 0.f}, {2.f, 1.f}};
        ///     std::vector<rr::datatypes::Vec2D> points2 = {{4.f, -1.f}, {6.f, 0.f}};
        ///     rec.log("segments", rr::LineStrips2D({points1, points2}));
        ///
        ///     // Log an extra rect to set the view bounds
        ///     rec.log("bounds", rr::Boxes2D::from_centers_and_sizes({{3.0f, 0.0f}},
        ///     {{8.0f, 6.0f}}));
        /// }
        /// ```
        struct LineStrips2D {
            /// All the actual 2D line strips that make up the batch.
            std::vector<rerun::components::LineStrip2D> strips;

            /// Optional radii for the line strips.
            std::optional<std::vector<rerun::components::Radius>> radii;

            /// Optional colors for the line strips.
            std::optional<std::vector<rerun::components::Color>> colors;

            /// Optional text labels for the line strips.
            std::optional<std::vector<rerun::components::Text>> labels;

            /// An optional floating point value that specifies the 2D drawing order of each line
            /// strip. Objects with higher values are drawn on top of those with lower values.
            std::optional<rerun::components::DrawOrder> draw_order;

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rerun::components::ClassId>> class_ids;

            /// Unique identifiers for each individual line strip in the batch.
            std::optional<std::vector<rerun::components::InstanceKey>> instance_keys;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];

          public:
            LineStrips2D() = default;

            LineStrips2D(std::vector<rerun::components::LineStrip2D> _strips)
                : strips(std::move(_strips)) {}

            LineStrips2D(rerun::components::LineStrip2D _strips) : strips(1, std::move(_strips)) {}

            /// Optional radii for the line strips.
            LineStrips2D& with_radii(std::vector<rerun::components::Radius> _radii) {
                radii = std::move(_radii);
                return *this;
            }

            /// Optional radii for the line strips.
            LineStrips2D& with_radii(rerun::components::Radius _radii) {
                radii = std::vector(1, std::move(_radii));
                return *this;
            }

            /// Optional colors for the line strips.
            LineStrips2D& with_colors(std::vector<rerun::components::Color> _colors) {
                colors = std::move(_colors);
                return *this;
            }

            /// Optional colors for the line strips.
            LineStrips2D& with_colors(rerun::components::Color _colors) {
                colors = std::vector(1, std::move(_colors));
                return *this;
            }

            /// Optional text labels for the line strips.
            LineStrips2D& with_labels(std::vector<rerun::components::Text> _labels) {
                labels = std::move(_labels);
                return *this;
            }

            /// Optional text labels for the line strips.
            LineStrips2D& with_labels(rerun::components::Text _labels) {
                labels = std::vector(1, std::move(_labels));
                return *this;
            }

            /// An optional floating point value that specifies the 2D drawing order of each line
            /// strip. Objects with higher values are drawn on top of those with lower values.
            LineStrips2D& with_draw_order(rerun::components::DrawOrder _draw_order) {
                draw_order = std::move(_draw_order);
                return *this;
            }

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            LineStrips2D& with_class_ids(std::vector<rerun::components::ClassId> _class_ids) {
                class_ids = std::move(_class_ids);
                return *this;
            }

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            LineStrips2D& with_class_ids(rerun::components::ClassId _class_ids) {
                class_ids = std::vector(1, std::move(_class_ids));
                return *this;
            }

            /// Unique identifiers for each individual line strip in the batch.
            LineStrips2D& with_instance_keys(
                std::vector<rerun::components::InstanceKey> _instance_keys
            ) {
                instance_keys = std::move(_instance_keys);
                return *this;
            }

            /// Unique identifiers for each individual line strip in the batch.
            LineStrips2D& with_instance_keys(rerun::components::InstanceKey _instance_keys) {
                instance_keys = std::vector(1, std::move(_instance_keys));
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return strips.size();
            }

            /// Collections all component lists into a list of component collections. *Attention:*
            /// The returned vector references this instance and does not take ownership of any
            /// data. Adding any new components to this archetype will invalidate the returned
            /// component lists!
            std::vector<AnonymousComponentBatch> as_component_batches() const;
        };
    } // namespace archetypes
} // namespace rerun
