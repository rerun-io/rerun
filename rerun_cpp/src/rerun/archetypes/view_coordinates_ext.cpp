#include "view_coordinates.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ViewCoordinatesExt {
            uint8_t coordinates[3];
#define ViewCoordinates ViewCoordinatesExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : xyz(rerun::components::ViewCoordinates(axis0, axis1, axis2)) {}

            // <BEGIN_GENERATED:declarations>
            // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates ULF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates UFL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LUF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LFU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FUL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FLU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates ULB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates UBL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LUB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LBU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BUL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BLU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates URF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates UFR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RUF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RFU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FUR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FRU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates URB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates UBR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RUB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RBU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BUR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BRU;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DLF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DFL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LDF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LFD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FDL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FLD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DLB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DBL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LDB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LBD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BDL;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BLD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DRF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DFR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RDF;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RFD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FDR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates FRD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DRB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates DBR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RDB;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RBD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BDR;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates BRD;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_X_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_X_DOWN;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_Y_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_Y_DOWN;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_Z_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates RIGHT_HAND_Z_DOWN;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_X_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_X_DOWN;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_Y_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_Y_DOWN;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_Z_UP;
            RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates LEFT_HAND_Z_DOWN;
            // <END_GENERATED:declarations>

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

        // <BEGIN_GENERATED:definitions>
        // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
        const ViewCoordinates ViewCoordinates::ULF =
            ViewCoordinates(rerun::components::ViewCoordinates::ULF);
        const ViewCoordinates ViewCoordinates::UFL =
            ViewCoordinates(rerun::components::ViewCoordinates::UFL);
        const ViewCoordinates ViewCoordinates::LUF =
            ViewCoordinates(rerun::components::ViewCoordinates::LUF);
        const ViewCoordinates ViewCoordinates::LFU =
            ViewCoordinates(rerun::components::ViewCoordinates::LFU);
        const ViewCoordinates ViewCoordinates::FUL =
            ViewCoordinates(rerun::components::ViewCoordinates::FUL);
        const ViewCoordinates ViewCoordinates::FLU =
            ViewCoordinates(rerun::components::ViewCoordinates::FLU);
        const ViewCoordinates ViewCoordinates::ULB =
            ViewCoordinates(rerun::components::ViewCoordinates::ULB);
        const ViewCoordinates ViewCoordinates::UBL =
            ViewCoordinates(rerun::components::ViewCoordinates::UBL);
        const ViewCoordinates ViewCoordinates::LUB =
            ViewCoordinates(rerun::components::ViewCoordinates::LUB);
        const ViewCoordinates ViewCoordinates::LBU =
            ViewCoordinates(rerun::components::ViewCoordinates::LBU);
        const ViewCoordinates ViewCoordinates::BUL =
            ViewCoordinates(rerun::components::ViewCoordinates::BUL);
        const ViewCoordinates ViewCoordinates::BLU =
            ViewCoordinates(rerun::components::ViewCoordinates::BLU);
        const ViewCoordinates ViewCoordinates::URF =
            ViewCoordinates(rerun::components::ViewCoordinates::URF);
        const ViewCoordinates ViewCoordinates::UFR =
            ViewCoordinates(rerun::components::ViewCoordinates::UFR);
        const ViewCoordinates ViewCoordinates::RUF =
            ViewCoordinates(rerun::components::ViewCoordinates::RUF);
        const ViewCoordinates ViewCoordinates::RFU =
            ViewCoordinates(rerun::components::ViewCoordinates::RFU);
        const ViewCoordinates ViewCoordinates::FUR =
            ViewCoordinates(rerun::components::ViewCoordinates::FUR);
        const ViewCoordinates ViewCoordinates::FRU =
            ViewCoordinates(rerun::components::ViewCoordinates::FRU);
        const ViewCoordinates ViewCoordinates::URB =
            ViewCoordinates(rerun::components::ViewCoordinates::URB);
        const ViewCoordinates ViewCoordinates::UBR =
            ViewCoordinates(rerun::components::ViewCoordinates::UBR);
        const ViewCoordinates ViewCoordinates::RUB =
            ViewCoordinates(rerun::components::ViewCoordinates::RUB);
        const ViewCoordinates ViewCoordinates::RBU =
            ViewCoordinates(rerun::components::ViewCoordinates::RBU);
        const ViewCoordinates ViewCoordinates::BUR =
            ViewCoordinates(rerun::components::ViewCoordinates::BUR);
        const ViewCoordinates ViewCoordinates::BRU =
            ViewCoordinates(rerun::components::ViewCoordinates::BRU);
        const ViewCoordinates ViewCoordinates::DLF =
            ViewCoordinates(rerun::components::ViewCoordinates::DLF);
        const ViewCoordinates ViewCoordinates::DFL =
            ViewCoordinates(rerun::components::ViewCoordinates::DFL);
        const ViewCoordinates ViewCoordinates::LDF =
            ViewCoordinates(rerun::components::ViewCoordinates::LDF);
        const ViewCoordinates ViewCoordinates::LFD =
            ViewCoordinates(rerun::components::ViewCoordinates::LFD);
        const ViewCoordinates ViewCoordinates::FDL =
            ViewCoordinates(rerun::components::ViewCoordinates::FDL);
        const ViewCoordinates ViewCoordinates::FLD =
            ViewCoordinates(rerun::components::ViewCoordinates::FLD);
        const ViewCoordinates ViewCoordinates::DLB =
            ViewCoordinates(rerun::components::ViewCoordinates::DLB);
        const ViewCoordinates ViewCoordinates::DBL =
            ViewCoordinates(rerun::components::ViewCoordinates::DBL);
        const ViewCoordinates ViewCoordinates::LDB =
            ViewCoordinates(rerun::components::ViewCoordinates::LDB);
        const ViewCoordinates ViewCoordinates::LBD =
            ViewCoordinates(rerun::components::ViewCoordinates::LBD);
        const ViewCoordinates ViewCoordinates::BDL =
            ViewCoordinates(rerun::components::ViewCoordinates::BDL);
        const ViewCoordinates ViewCoordinates::BLD =
            ViewCoordinates(rerun::components::ViewCoordinates::BLD);
        const ViewCoordinates ViewCoordinates::DRF =
            ViewCoordinates(rerun::components::ViewCoordinates::DRF);
        const ViewCoordinates ViewCoordinates::DFR =
            ViewCoordinates(rerun::components::ViewCoordinates::DFR);
        const ViewCoordinates ViewCoordinates::RDF =
            ViewCoordinates(rerun::components::ViewCoordinates::RDF);
        const ViewCoordinates ViewCoordinates::RFD =
            ViewCoordinates(rerun::components::ViewCoordinates::RFD);
        const ViewCoordinates ViewCoordinates::FDR =
            ViewCoordinates(rerun::components::ViewCoordinates::FDR);
        const ViewCoordinates ViewCoordinates::FRD =
            ViewCoordinates(rerun::components::ViewCoordinates::FRD);
        const ViewCoordinates ViewCoordinates::DRB =
            ViewCoordinates(rerun::components::ViewCoordinates::DRB);
        const ViewCoordinates ViewCoordinates::DBR =
            ViewCoordinates(rerun::components::ViewCoordinates::DBR);
        const ViewCoordinates ViewCoordinates::RDB =
            ViewCoordinates(rerun::components::ViewCoordinates::RDB);
        const ViewCoordinates ViewCoordinates::RBD =
            ViewCoordinates(rerun::components::ViewCoordinates::RBD);
        const ViewCoordinates ViewCoordinates::BDR =
            ViewCoordinates(rerun::components::ViewCoordinates::BDR);
        const ViewCoordinates ViewCoordinates::BRD =
            ViewCoordinates(rerun::components::ViewCoordinates::BRD);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_X_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_X_UP);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_X_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_X_DOWN);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Y_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_Y_UP);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Y_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_Y_DOWN);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Z_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_Z_UP);
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Z_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::RIGHT_HAND_Z_DOWN);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_X_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_X_UP);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_X_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_X_DOWN);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Y_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_Y_UP);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Y_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_Y_DOWN);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Z_UP =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_Z_UP);
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Z_DOWN =
            ViewCoordinates(rerun::components::ViewCoordinates::LEFT_HAND_Z_DOWN);
        // <END_GENERATED:definitions>

    } // namespace archetypes
} // namespace rerun
