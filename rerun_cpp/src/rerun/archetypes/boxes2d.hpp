// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/boxes2d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/draw_order.hpp"
#include "../components/half_size2d.hpp"
#include "../components/position2d.hpp"
#include "../components/radius.hpp"
#include "../components/text.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: 2D boxes with half-extents and optional center, rotations, rotations, colors etc.
    ///
    /// ## Example
    ///
    /// ### Simple 2D boxes
    /// ![image](https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_box2d");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log("simple", rerun::Boxes2D::from_mins_and_sizes({{-1.f, -1.f}}, {{2.f, 2.f}}));
    /// }
    /// ```
    struct Boxes2D {
        /// All half-extents that make up the batch of boxes.
        Collection<rerun::components::HalfSize2D> half_sizes;

        /// Optional center positions of the boxes.
        std::optional<Collection<rerun::components::Position2D>> centers;

        /// Optional colors for the boxes.
        std::optional<Collection<rerun::components::Color>> colors;

        /// Optional radii for the lines that make up the boxes.
        std::optional<Collection<rerun::components::Radius>> radii;

        /// Optional text labels for the boxes.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        std::optional<Collection<rerun::components::Text>> labels;

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        ///
        /// The default for 2D boxes is 10.0.
        std::optional<rerun::components::DrawOrder> draw_order;

        /// Optional `ClassId`s for the boxes.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        std::optional<Collection<rerun::components::ClassId>> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Boxes2DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        // Extensions to generated type defined in 'boxes2d_ext.cpp'

        /// Creates new `Boxes2D` with `half_sizes` centered around the local origin.
        static Boxes2D from_half_sizes(Collection<components::HalfSize2D> half_sizes) {
            Boxes2D boxes;
            boxes.half_sizes = std::move(half_sizes);
            return boxes;
        }

        /// Creates new `Boxes2D` with `centers` and `half_sizes`.
        static Boxes2D from_centers_and_half_sizes(
            Collection<components::Position2D> centers,
            Collection<components::HalfSize2D> half_sizes
        ) {
            Boxes2D boxes;
            boxes.half_sizes = std::move(half_sizes);
            boxes.centers = std::move(centers);
            return boxes;
        }

        /// Creates new `Boxes2D` with `half_sizes` created from (full) sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the
        /// input data.
        static Boxes2D from_sizes(const std::vector<datatypes::Vec2D>& sizes);

        /// Creates new `Boxes2D` with `centers` and `half_sizes` created from centers and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        static Boxes2D from_centers_and_sizes(
            Collection<components::Position2D> centers, const std::vector<datatypes::Vec2D>& sizes
        ) {
            Boxes2D boxes = from_sizes(std::move(sizes));
            boxes.centers = std::move(centers);
            return boxes;
        }

        /// Creates new `Boxes2D` with `half_sizes` and `centers` created from minimums and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        static Boxes2D from_mins_and_sizes(
            const std::vector<datatypes::Vec2D>& mins, const std::vector<datatypes::Vec2D>& sizes
        );

      public:
        Boxes2D() = default;
        Boxes2D(Boxes2D&& other) = default;

        /// Optional center positions of the boxes.
        Boxes2D with_centers(Collection<rerun::components::Position2D> _centers) && {
            centers = std::move(_centers);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional colors for the boxes.
        Boxes2D with_colors(Collection<rerun::components::Color> _colors) && {
            colors = std::move(_colors);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional radii for the lines that make up the boxes.
        Boxes2D with_radii(Collection<rerun::components::Radius> _radii) && {
            radii = std::move(_radii);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional text labels for the boxes.
        ///
        /// If there's a single label present, it will be placed at the center of the entity.
        /// Otherwise, each instance will have its own label.
        Boxes2D with_labels(Collection<rerun::components::Text> _labels) && {
            labels = std::move(_labels);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional floating point value that specifies the 2D drawing order.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        ///
        /// The default for 2D boxes is 10.0.
        Boxes2D with_draw_order(rerun::components::DrawOrder _draw_order) && {
            draw_order = std::move(_draw_order);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional `ClassId`s for the boxes.
        ///
        /// The class ID provides colors and labels if not specified explicitly.
        Boxes2D with_class_ids(Collection<rerun::components::ClassId> _class_ids) && {
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
    struct AsComponents<archetypes::Boxes2D> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::Boxes2D& archetype);
    };
} // namespace rerun
