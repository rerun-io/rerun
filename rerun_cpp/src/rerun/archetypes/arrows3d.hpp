// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/arrows3d.fbs".

#pragma once

#include "../arrow.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/label.hpp"
#include "../components/origin3d.hpp"
#include "../components/radius.hpp"
#include "../components/vector3d.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// A batch of 3D arrows with optional colors, radii, labels, etc.
        ///
        /// ## Example
        ///
        ///```
        ///// Log a batch of 3D arrows.
        ///
        /// #include <rerun.hpp>
        ///
        /// #include <cmath>
        /// #include <numeric>
        ///
        /// namespace rr = rerun;
        ///
        /// double rnd(double v) {
        ///    return round(v * 100.0f) / 100.0;
        /// }
        ///
        /// int main() {
        ///    auto rec = rr::RecordingStream("rerun_example_arrow3d");
        ///    rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///    std::vector<rr::components::Vector3D> vectors;
        ///    std::vector<rr::components::Color> colors;
        ///
        ///    for (int i = 0; i <100; ++i) {
        ///        double angle = rnd(2.0 * M_PI * i * 0.01f);
        ///        double length = rnd(log2f(i + 1));
        ///        vectors.push_back({(float)(length * sin(angle)), 0.0, (float)(length *
        ///        cos(angle))});
        ///
        ///        uint8_t c = static_cast<uint8_t>(round(angle / (2.0 * M_PI) * 255.0));
        ///        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
        ///    }
        ///
        ///    rec.log("arrows", rr::Arrows3D(vectors).with_colors(colors));
        /// }
        ///```
        struct Arrows3D {
            /// All the vectors for each arrow in the batch.
            std::vector<rerun::components::Vector3D> vectors;

            /// All the origin points for each arrow in the batch.
            std::optional<std::vector<rerun::components::Origin3D>> origins;

            /// Optional radii for the arrows.
            ///
            /// The shaft is rendered as a line with `radius = 0.5 * radius`.
            /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
            std::optional<std::vector<rerun::components::Radius>> radii;

            /// Optional colors for the points.
            std::optional<std::vector<rerun::components::Color>> colors;

            /// Optional text labels for the arrows.
            std::optional<std::vector<rerun::components::Label>> labels;

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rerun::components::ClassId>> class_ids;

            /// Unique identifiers for each individual point in the batch.
            std::optional<std::vector<rerun::components::InstanceKey>> instance_keys;

          public:
            Arrows3D() = default;

            Arrows3D(std::vector<rerun::components::Vector3D> _vectors)
                : vectors(std::move(_vectors)) {}

            Arrows3D(rerun::components::Vector3D _vectors) : vectors(1, std::move(_vectors)) {}

            /// All the origin points for each arrow in the batch.
            Arrows3D& with_origins(std::vector<rerun::components::Origin3D> _origins) {
                origins = std::move(_origins);
                return *this;
            }

            /// All the origin points for each arrow in the batch.
            Arrows3D& with_origins(rerun::components::Origin3D _origins) {
                origins = std::vector(1, std::move(_origins));
                return *this;
            }

            /// Optional radii for the arrows.
            ///
            /// The shaft is rendered as a line with `radius = 0.5 * radius`.
            /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
            Arrows3D& with_radii(std::vector<rerun::components::Radius> _radii) {
                radii = std::move(_radii);
                return *this;
            }

            /// Optional radii for the arrows.
            ///
            /// The shaft is rendered as a line with `radius = 0.5 * radius`.
            /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
            Arrows3D& with_radii(rerun::components::Radius _radii) {
                radii = std::vector(1, std::move(_radii));
                return *this;
            }

            /// Optional colors for the points.
            Arrows3D& with_colors(std::vector<rerun::components::Color> _colors) {
                colors = std::move(_colors);
                return *this;
            }

            /// Optional colors for the points.
            Arrows3D& with_colors(rerun::components::Color _colors) {
                colors = std::vector(1, std::move(_colors));
                return *this;
            }

            /// Optional text labels for the arrows.
            Arrows3D& with_labels(std::vector<rerun::components::Label> _labels) {
                labels = std::move(_labels);
                return *this;
            }

            /// Optional text labels for the arrows.
            Arrows3D& with_labels(rerun::components::Label _labels) {
                labels = std::vector(1, std::move(_labels));
                return *this;
            }

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Arrows3D& with_class_ids(std::vector<rerun::components::ClassId> _class_ids) {
                class_ids = std::move(_class_ids);
                return *this;
            }

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Arrows3D& with_class_ids(rerun::components::ClassId _class_ids) {
                class_ids = std::vector(1, std::move(_class_ids));
                return *this;
            }

            /// Unique identifiers for each individual point in the batch.
            Arrows3D& with_instance_keys(std::vector<rerun::components::InstanceKey> _instance_keys
            ) {
                instance_keys = std::move(_instance_keys);
                return *this;
            }

            /// Unique identifiers for each individual point in the batch.
            Arrows3D& with_instance_keys(rerun::components::InstanceKey _instance_keys) {
                instance_keys = std::vector(1, std::move(_instance_keys));
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return vectors.size();
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
