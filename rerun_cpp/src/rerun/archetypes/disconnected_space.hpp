// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs"

#pragma once

#include "../components/disconnected_space.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rerun {
    namespace archetypes {
        /// Specifies that the entity path at which this is logged is disconnected from its parent.
        ///
        /// This is useful for specifying that a subgraph is independent of the rest of the scene.
        ///
        /// If a transform or pinhole is logged on the same path, this archetype's components
        /// will be ignored.
        ///
        /// ## Example
        ///
        ///```
        ///// Disconnect two spaces.
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///    auto rr_stream = rr::RecordingStream("disconnected_space");
        ///    rr_stream.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///    // These two points can be projected into the same space..
        ///    rr_stream.log("world/room1/point", rr::Points3D(rr::datatypes::Vec3D{0.0f, 0.0f,
        ///    0.0f})); rr_stream.log("world/room2/point",
        ///    rr::Points3D(rr::datatypes::Vec3D{1.0f, 1.0f, 1.0f}));
        ///
        ///    // ..but this one lives in a completely separate space!
        ///    rr_stream.log("world/wormhole", rr::DisconnectedSpace(true));
        ///    rr_stream.log("world/wormhole/point",
        ///    rr::Points3D(rr::datatypes::Vec3D{2.0f, 2.0f, 2.0f}));
        /// }
        ///```
        struct DisconnectedSpace {
            rerun::components::DisconnectedSpace disconnected_space;

          public:
            DisconnectedSpace() = default;

            DisconnectedSpace(rerun::components::DisconnectedSpace _disconnected_space)
                : disconnected_space(std::move(_disconnected_space)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
