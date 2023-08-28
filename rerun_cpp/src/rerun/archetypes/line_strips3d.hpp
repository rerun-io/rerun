// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips3d.fbs"

#pragma once

#include "../arrow.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/label.hpp"
#include "../components/line_strip3d.hpp"
#include "../components/radius.hpp"
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
        /// ## Example
        ///
        /// Many strips:
        ///```ignore
        ///// Log a batch of 3d line strips.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///    auto rr_stream = rr::RecordingStream("line_strip3d");
        ///    rr_stream.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///    std::vector<rr::datatypes::Vec3D> strip1 = {
        ///        {0.f, 0.f, 2.f},
        ///        {1.f, 0.f, 2.f},
        ///        {1.f, 1.f, 2.f},
        ///        {0.f, 1.f, 2.f},
        ///    };
        ///    std::vector<rr::datatypes::Vec3D> strip2 = {
        ///        {0.f, 0.f, 0.f},
        ///        {0.f, 0.f, 1.f},
        ///        {1.f, 0.f, 0.f},
        ///        {1.f, 0.f, 1.f},
        ///        {1.f, 1.f, 0.f},
        ///        {1.f, 1.f, 1.f},
        ///        {0.f, 1.f, 0.f},
        ///        {0.f, 1.f, 1.f},
        ///    };
        ///    rr_stream.log(
        ///        "strips",
        ///        rr::LineStrips3D({strip1, strip2})
        ///            .with_colors({0xFF0000FF, 0x00FF00FF})
        ///            .with_radii({0.025f, 0.005f})
        ///            .with_labels({"one strip here", "and one strip there"})
        ///    );
        /// }
        ///```
        ///
        /// Many individual segments:
        ///```ignore
        ///// Log a simple set of line segments.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///    auto rr_stream = rr::RecordingStream("line_segments3d");
        ///    rr_stream.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///    std::vector<rr::datatypes::Vec3D> points = {
        ///        {0.f, 0.f, 0.f},
        ///        {0.f, 0.f, 1.f},
        ///        {1.f, 0.f, 0.f},
        ///        {1.f, 0.f, 1.f},
        ///        {1.f, 1.f, 0.f},
        ///        {1.f, 1.f, 1.f},
        ///        {0.f, 1.f, 0.f},
        ///        {0.f, 1.f, 1.f},
        ///    };
        ///    rr_stream.log("segments", rr::LineStrips3D(points));
        /// }
        ///```
        struct LineStrips3D {
            /// All the actual 3D line strips that make up the batch.
            std::vector<rerun::components::LineStrip3D> strips;

            /// Optional radii for the line strips.
            std::optional<std::vector<rerun::components::Radius>> radii;

            /// Optional colors for the line strips.
            std::optional<std::vector<rerun::components::Color>> colors;

            /// Optional text labels for the line strips.
            std::optional<std::vector<rerun::components::Label>> labels;

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rerun::components::ClassId>> class_ids;

            /// Unique identifiers for each individual line strip in the batch.
            std::optional<std::vector<rerun::components::InstanceKey>> instance_keys;

          public:
            LineStrips3D() = default;

            LineStrips3D(std::vector<rerun::components::LineStrip3D> _strips)
                : strips(std::move(_strips)) {}

            LineStrips3D(rerun::components::LineStrip3D _strips) : strips(1, std::move(_strips)) {}

            /// Optional radii for the line strips.
            LineStrips3D& with_radii(std::vector<rerun::components::Radius> _radii) {
                radii = std::move(_radii);
                return *this;
            }

            /// Optional radii for the line strips.
            LineStrips3D& with_radii(rerun::components::Radius _radii) {
                radii = std::vector(1, std::move(_radii));
                return *this;
            }

            /// Optional colors for the line strips.
            LineStrips3D& with_colors(std::vector<rerun::components::Color> _colors) {
                colors = std::move(_colors);
                return *this;
            }

            /// Optional colors for the line strips.
            LineStrips3D& with_colors(rerun::components::Color _colors) {
                colors = std::vector(1, std::move(_colors));
                return *this;
            }

            /// Optional text labels for the line strips.
            LineStrips3D& with_labels(std::vector<rerun::components::Label> _labels) {
                labels = std::move(_labels);
                return *this;
            }

            /// Optional text labels for the line strips.
            LineStrips3D& with_labels(rerun::components::Label _labels) {
                labels = std::vector(1, std::move(_labels));
                return *this;
            }

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            LineStrips3D& with_class_ids(std::vector<rerun::components::ClassId> _class_ids) {
                class_ids = std::move(_class_ids);
                return *this;
            }

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            LineStrips3D& with_class_ids(rerun::components::ClassId _class_ids) {
                class_ids = std::vector(1, std::move(_class_ids));
                return *this;
            }

            /// Unique identifiers for each individual line strip in the batch.
            LineStrips3D& with_instance_keys(
                std::vector<rerun::components::InstanceKey> _instance_keys
            ) {
                instance_keys = std::move(_instance_keys);
                return *this;
            }

            /// Unique identifiers for each individual line strip in the batch.
            LineStrips3D& with_instance_keys(rerun::components::InstanceKey _instance_keys) {
                instance_keys = std::vector(1, std::move(_instance_keys));
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return strips.size();
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
