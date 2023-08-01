// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs"

#pragma once

#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/keypoint_id.hpp"
#include "../components/label.hpp"
#include "../components/point3d.hpp"
#include "../components/radius.hpp"
#include "../data_cell.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rr {
    namespace archetypes {
        /// A 3D point cloud with positions and optional colors, radii, labels, etc.
        struct Points3D {
            /// All the actual 3D points that make up the point cloud.
            std::vector<rr::components::Point3D> points;

            /// Optional radii for the points, effectively turning them into circles.
            std::optional<std::vector<rr::components::Radius>> radii;

            /// Optional colors for the points.
            std::optional<std::vector<rr::components::Color>> colors;

            /// Optional text labels for the points.
            std::optional<std::vector<rr::components::Label>> labels;

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<std::vector<rr::components::ClassId>> class_ids;

            /// Optional keypoint IDs for the points, identifying them within a class.
            ///
            /// If keypoint IDs are passed in but no class IDs were specified, the class ID will
            /// default to 0.
            /// This is useful to identify points within a single classification (which is
            /// identified with `class_id`). E.g. the classification might be 'Person' and the
            /// keypoints refer to joints on a detected skeleton.
            std::optional<std::vector<rr::components::KeypointId>> keypoint_ids;

            /// Unique identifiers for each individual point in the batch.
            std::optional<std::vector<rr::components::InstanceKey>> instance_keys;

          public:
            Points3D(std::vector<rr::components::Point3D> points) : points(std::move(points)) {}

            /// Optional radii for the points, effectively turning them into circles.
            Points3D& with_radii(std::vector<rr::components::Radius> radii) {
                this->radii = std::move(radii);
                return *this;
            }

            /// Optional colors for the points.
            Points3D& with_colors(std::vector<rr::components::Color> colors) {
                this->colors = std::move(colors);
                return *this;
            }

            /// Optional text labels for the points.
            Points3D& with_labels(std::vector<rr::components::Label> labels) {
                this->labels = std::move(labels);
                return *this;
            }

            /// Optional class Ids for the points.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            Points3D& with_class_ids(std::vector<rr::components::ClassId> class_ids) {
                this->class_ids = std::move(class_ids);
                return *this;
            }

            /// Optional keypoint IDs for the points, identifying them within a class.
            ///
            /// If keypoint IDs are passed in but no class IDs were specified, the class ID will
            /// default to 0.
            /// This is useful to identify points within a single classification (which is
            /// identified with `class_id`). E.g. the classification might be 'Person' and the
            /// keypoints refer to joints on a detected skeleton.
            Points3D& with_keypoint_ids(std::vector<rr::components::KeypointId> keypoint_ids) {
                this->keypoint_ids = std::move(keypoint_ids);
                return *this;
            }

            /// Unique identifiers for each individual point in the batch.
            Points3D& with_instance_keys(std::vector<rr::components::InstanceKey> instance_keys) {
                this->instance_keys = std::move(instance_keys);
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return points.size();
            }

            /// Creates a list of Rerun DataCell from this archetype.
            arrow::Result<std::vector<rr::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rr
