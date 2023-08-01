// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/arrows3d.fbs"

#pragma once

#include "../components/arrow3d.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/label.hpp"
#include "../components/radius.hpp"
#include "../data_cell.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rr {
    namespace archetypes {
        /// A batch of 3D arrows with optional colors, radii, labels, etc.
        struct Arrows3D {
            /// All the individual arrows that make up the batch.
            std::vector<rr::components::Arrow3D> arrows;

            /// Optional radii for the arrows.
            ///
            /// The shaft is rendered as a cylinder with `radius = 0.5 * radius`.
            /// The tip is rendered as a cone with `height = 2.0 * radius` and `radius = 1.0 *
            /// radius`.
            std::optional<std::vector<rr::components::Radius>> radii;

            /// Optional colors for the points.
            std::optional<std::vector<rr::components::Color>> colors;

            /// Optional text labels for the arrows.
            std::optional<std::vector<rr::components::Label>> labels;

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rr::components::ClassId>> class_ids;

            /// Unique identifiers for each individual point in the batch.
            std::optional<std::vector<rr::components::InstanceKey>> instance_keys;

          public:
            Arrows3D(std::vector<rr::components::Arrow3D> arrows) : arrows(std::move(arrows)) {}

            /// Optional radii for the arrows.
            ///
            /// The shaft is rendered as a cylinder with `radius = 0.5 * radius`.
            /// The tip is rendered as a cone with `height = 2.0 * radius` and `radius = 1.0 *
            /// radius`.
            Arrows3D& with_radii(std::vector<rr::components::Radius> radii) {
                this->radii = std::move(radii);
                return *this;
            }

            /// Optional colors for the points.
            Arrows3D& with_colors(std::vector<rr::components::Color> colors) {
                this->colors = std::move(colors);
                return *this;
            }

            /// Optional text labels for the arrows.
            Arrows3D& with_labels(std::vector<rr::components::Label> labels) {
                this->labels = std::move(labels);
                return *this;
            }

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Arrows3D& with_class_ids(std::vector<rr::components::ClassId> class_ids) {
                this->class_ids = std::move(class_ids);
                return *this;
            }

            /// Unique identifiers for each individual point in the batch.
            Arrows3D& with_instance_keys(std::vector<rr::components::InstanceKey> instance_keys) {
                this->instance_keys = std::move(instance_keys);
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return arrows.size();
            }

            /// Creates a list of Rerun DataCell from this archetype.
            arrow::Result<std::vector<rr::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rr
