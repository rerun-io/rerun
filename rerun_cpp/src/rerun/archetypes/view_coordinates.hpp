// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/view_coordinates.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/view_coordinates.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// How we interpret the coordinate system of an entity/space.
        struct ViewCoordinates {
            rerun::components::ViewCoordinates coordinates;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];

          public:
            // Extensions to generated type defined in 'view_coordinates_ext.cpp'

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : coordinates(rerun::components::ViewCoordinates(axis0, axis1, axis2)) {}

            // <BEGIN_GENERATED:declarations>
            // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
            static const rerun::archetypes::ViewCoordinates ULF;
            static const rerun::archetypes::ViewCoordinates UFL;
            static const rerun::archetypes::ViewCoordinates LUF;
            static const rerun::archetypes::ViewCoordinates LFU;
            static const rerun::archetypes::ViewCoordinates FUL;
            static const rerun::archetypes::ViewCoordinates FLU;
            static const rerun::archetypes::ViewCoordinates ULB;
            static const rerun::archetypes::ViewCoordinates UBL;
            static const rerun::archetypes::ViewCoordinates LUB;
            static const rerun::archetypes::ViewCoordinates LBU;
            static const rerun::archetypes::ViewCoordinates BUL;
            static const rerun::archetypes::ViewCoordinates BLU;
            static const rerun::archetypes::ViewCoordinates URF;
            static const rerun::archetypes::ViewCoordinates UFR;
            static const rerun::archetypes::ViewCoordinates RUF;
            static const rerun::archetypes::ViewCoordinates RFU;
            static const rerun::archetypes::ViewCoordinates FUR;
            static const rerun::archetypes::ViewCoordinates FRU;
            static const rerun::archetypes::ViewCoordinates URB;
            static const rerun::archetypes::ViewCoordinates UBR;
            static const rerun::archetypes::ViewCoordinates RUB;
            static const rerun::archetypes::ViewCoordinates RBU;
            static const rerun::archetypes::ViewCoordinates BUR;
            static const rerun::archetypes::ViewCoordinates BRU;
            static const rerun::archetypes::ViewCoordinates DLF;
            static const rerun::archetypes::ViewCoordinates DFL;
            static const rerun::archetypes::ViewCoordinates LDF;
            static const rerun::archetypes::ViewCoordinates LFD;
            static const rerun::archetypes::ViewCoordinates FDL;
            static const rerun::archetypes::ViewCoordinates FLD;
            static const rerun::archetypes::ViewCoordinates DLB;
            static const rerun::archetypes::ViewCoordinates DBL;
            static const rerun::archetypes::ViewCoordinates LDB;
            static const rerun::archetypes::ViewCoordinates LBD;
            static const rerun::archetypes::ViewCoordinates BDL;
            static const rerun::archetypes::ViewCoordinates BLD;
            static const rerun::archetypes::ViewCoordinates DRF;
            static const rerun::archetypes::ViewCoordinates DFR;
            static const rerun::archetypes::ViewCoordinates RDF;
            static const rerun::archetypes::ViewCoordinates RFD;
            static const rerun::archetypes::ViewCoordinates FDR;
            static const rerun::archetypes::ViewCoordinates FRD;
            static const rerun::archetypes::ViewCoordinates DRB;
            static const rerun::archetypes::ViewCoordinates DBR;
            static const rerun::archetypes::ViewCoordinates RDB;
            static const rerun::archetypes::ViewCoordinates RBD;
            static const rerun::archetypes::ViewCoordinates BDR;
            static const rerun::archetypes::ViewCoordinates BRD;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_POS_X_UP;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_NEG_X_UP;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_POS_Y_UP;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_NEG_Y_UP;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_POS_Z_UP;
            static const rerun::archetypes::ViewCoordinates RIGHT_HAND_NEG_Z_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_POS_X_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_NEG_X_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_POS_Y_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_NEG_Y_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_POS_Z_UP;
            static const rerun::archetypes::ViewCoordinates LEFT_HAND_NEG_Z_UP;
            // <END_GENERATED:declarations>

          public:
            ViewCoordinates() = default;

            ViewCoordinates(rerun::components::ViewCoordinates _coordinates)
                : coordinates(std::move(_coordinates)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Collections all component lists into a list of component collections. *Attention:*
            /// The returned vector references this instance and does not take ownership of any
            /// data. Adding any new components to this archetype will invalidate the returned
            /// component lists!
            std::vector<AnonymousComponentBatch> as_component_batches() const;
        };
    } // namespace archetypes
} // namespace rerun
