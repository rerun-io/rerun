// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/boxes3d.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/half_sizes3d.hpp"
#include "../components/instance_key.hpp"
#include "../components/position3d.hpp"
#include "../components/radius.hpp"
#include "../components/rotation3d.hpp"
#include "../components/text.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// A batch of 3d boxes with half-extents and optional center, rotations, rotations, colors
        /// etc.
        ///
        /// ## Examples
        ///
        /// ### Simple 3D boxes
        /// ```cpp,ignore
        /// // Log a single 3D box.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///     auto rec = rr::RecordingStream("rerun_example_box3d_simple");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     rec.log("simple", rr::Boxes3D::from_half_sizes({{2.f, 2.f, 1.0f}}));
        /// }
        /// ```
        ///
        /// ### Batch of 3D boxes
        /// ```cpp,ignore
        /// // Log a batch of oriented bounding boxes.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///     auto rec = rr::RecordingStream("rerun_example_box3d_batch");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     rec.log(
        ///         "batch",
        ///         rr::Boxes3D::from_centers_and_half_sizes(
        ///             {{2.0f, 0.0f, 0.0f}, {-2.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 2.0f}},
        ///             {{2.0f, 2.0f, 1.0f}, {1.0f, 1.0f, 0.5f}, {2.0f, 0.5f, 1.0f}}
        ///         )
        ///             .with_rotations({
        ///                 rr::datatypes::Quaternion::IDENTITY,
        ///                 rr::datatypes::Quaternion(0.0f, 0.0f, 0.382683f, 0.923880f), // 45
        ///                 degrees around Z rr::datatypes::RotationAxisAngle(
        ///                     {0.0f, 1.0f, 0.0f},
        ///                     rr::datatypes::Angle::degrees(30.0f)
        ///                 ),
        ///             })
        ///             .with_radii(0.025f)
        ///             .with_colors({
        ///                 rr::datatypes::Color(255, 0, 0),
        ///                 rr::datatypes::Color(0, 255, 0),
        ///                 rr::datatypes::Color(0, 0, 255),
        ///             })
        ///             .with_labels({"red", "green", "blue"})
        ///     );
        /// }
        /// ```
        struct Boxes3D {
            /// All half-extents that make up the batch of boxes.
            std::vector<rerun::components::HalfSizes3D> half_sizes;

            /// Optional center positions of the boxes.
            std::optional<std::vector<rerun::components::Position3D>> centers;

            std::optional<std::vector<rerun::components::Rotation3D>> rotations;

            /// Optional colors for the boxes.
            std::optional<std::vector<rerun::components::Color>> colors;

            /// Optional radii for the lines that make up the boxes.
            std::optional<std::vector<rerun::components::Radius>> radii;

            /// Optional text labels for the boxes.
            std::optional<std::vector<rerun::components::Text>> labels;

            /// Optional `ClassId`s for the boxes.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rerun::components::ClassId>> class_ids;

            /// Unique identifiers for each individual boxes in the batch.
            std::optional<std::vector<rerun::components::InstanceKey>> instance_keys;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];

          public:
            // Extensions to generated type defined in 'boxes3d_ext.cpp'

            /// Creates new `Boxes3D` with `half_sizes` centered around the local origin.
            static Boxes3D from_half_sizes(std::vector<components::HalfSizes3D> _half_sizes) {
                Boxes3D boxes;
                boxes.half_sizes = std::move(_half_sizes);
                return boxes;
            }

            /// Creates new `Boxes3D` with `centers` and `half_sizes`.
            static Boxes3D from_centers_and_half_sizes(
                std::vector<components::Position3D> _centers,
                std::vector<components::HalfSizes3D> _half_sizes
            ) {
                return Boxes3D::from_half_sizes(std::move(_half_sizes))
                    .with_centers(std::move(_centers));
            }

            /// Creates new `Boxes3D` with `half_sizes` created from (full) sizes.
            ///
            /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the
            /// input data.
            static Boxes3D from_sizes(const std::vector<datatypes::Vec3D>& sizes);

            /// Creates new `Boxes3D` with `centers` and `half_sizes` created from centers and
            /// (full) sizes.
            ///
            /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and
            /// half-sizes from the input data.
            static Boxes3D from_centers_and_sizes(
                std::vector<components::Position3D> centers,
                const std::vector<datatypes::Vec3D>& sizes
            ) {
                return from_sizes(sizes).with_centers(std::move(centers));
            }

            /// Creates new `Boxes3D` with `half_sizes` and `centers` created from minimums and
            /// (full) sizes.
            ///
            /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and
            /// half-sizes from the input data.
            static Boxes3D from_mins_and_sizes(
                const std::vector<datatypes::Vec3D>& mins,
                const std::vector<datatypes::Vec3D>& sizes
            );

          public:
            Boxes3D() = default;

            /// Optional center positions of the boxes.
            Boxes3D& with_centers(std::vector<rerun::components::Position3D> _centers) {
                centers = std::move(_centers);
                return *this;
            }

            /// Optional center positions of the boxes.
            Boxes3D& with_centers(rerun::components::Position3D _centers) {
                centers = std::vector(1, std::move(_centers));
                return *this;
            }

            Boxes3D& with_rotations(std::vector<rerun::components::Rotation3D> _rotations) {
                rotations = std::move(_rotations);
                return *this;
            }

            Boxes3D& with_rotations(rerun::components::Rotation3D _rotations) {
                rotations = std::vector(1, std::move(_rotations));
                return *this;
            }

            /// Optional colors for the boxes.
            Boxes3D& with_colors(std::vector<rerun::components::Color> _colors) {
                colors = std::move(_colors);
                return *this;
            }

            /// Optional colors for the boxes.
            Boxes3D& with_colors(rerun::components::Color _colors) {
                colors = std::vector(1, std::move(_colors));
                return *this;
            }

            /// Optional radii for the lines that make up the boxes.
            Boxes3D& with_radii(std::vector<rerun::components::Radius> _radii) {
                radii = std::move(_radii);
                return *this;
            }

            /// Optional radii for the lines that make up the boxes.
            Boxes3D& with_radii(rerun::components::Radius _radii) {
                radii = std::vector(1, std::move(_radii));
                return *this;
            }

            /// Optional text labels for the boxes.
            Boxes3D& with_labels(std::vector<rerun::components::Text> _labels) {
                labels = std::move(_labels);
                return *this;
            }

            /// Optional text labels for the boxes.
            Boxes3D& with_labels(rerun::components::Text _labels) {
                labels = std::vector(1, std::move(_labels));
                return *this;
            }

            /// Optional `ClassId`s for the boxes.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Boxes3D& with_class_ids(std::vector<rerun::components::ClassId> _class_ids) {
                class_ids = std::move(_class_ids);
                return *this;
            }

            /// Optional `ClassId`s for the boxes.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Boxes3D& with_class_ids(rerun::components::ClassId _class_ids) {
                class_ids = std::vector(1, std::move(_class_ids));
                return *this;
            }

            /// Unique identifiers for each individual boxes in the batch.
            Boxes3D& with_instance_keys(std::vector<rerun::components::InstanceKey> _instance_keys
            ) {
                instance_keys = std::move(_instance_keys);
                return *this;
            }

            /// Unique identifiers for each individual boxes in the batch.
            Boxes3D& with_instance_keys(rerun::components::InstanceKey _instance_keys) {
                instance_keys = std::vector(1, std::move(_instance_keys));
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return half_sizes.size();
            }

            /// Creates an `AnonymousComponentBatch` out of the associated indicator component. This
            /// allows for associating arbitrary indicator components with arbitrary data. Check out
            /// the `manual_indicator` API example to see what's possible.
            static AnonymousComponentBatch indicator();

            /// Collections all component lists into a list of component collections. *Attention:*
            /// The returned vector references this instance and does not take ownership of any
            /// data. Adding any new components to this archetype will invalidate the returned
            /// component lists!
            std::vector<AnonymousComponentBatch> as_component_batches() const;
        };
    } // namespace archetypes
} // namespace rerun
