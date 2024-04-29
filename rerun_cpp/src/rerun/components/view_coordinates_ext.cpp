#include "view_coordinates.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ViewCoordinatesExt {
            uint8_t coordinates[3];
#define ViewCoordinates ViewCoordinatesExt

            // <CODEGEN_COPY_TO_HEADER>

            enum ViewDir : uint8_t {
                Up = 1,
                Down = 2,
                Right = 3,
                Left = 4,
                Forward = 5,
                Back = 6,
            };

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : coordinates{axis0, axis1, axis2} {}

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(ViewDir axis0, ViewDir axis1, ViewDir axis2)
                : coordinates{axis0, axis1, axis2} {}

            // <BEGIN_GENERATED:declarations>
            // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
            /// X=Up, Y=Left, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates ULF;

            /// X=Up, Y=Forward, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UFL;

            /// X=Left, Y=Up, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LUF;

            /// X=Left, Y=Forward, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LFU;

            /// X=Forward, Y=Up, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FUL;

            /// X=Forward, Y=Left, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FLU;

            /// X=Up, Y=Left, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates ULB;

            /// X=Up, Y=Back, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UBL;

            /// X=Left, Y=Up, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LUB;

            /// X=Left, Y=Back, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LBU;

            /// X=Back, Y=Up, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BUL;

            /// X=Back, Y=Left, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BLU;

            /// X=Up, Y=Right, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates URF;

            /// X=Up, Y=Forward, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UFR;

            /// X=Right, Y=Up, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RUF;

            /// X=Right, Y=Forward, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RFU;

            /// X=Forward, Y=Up, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FUR;

            /// X=Forward, Y=Right, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FRU;

            /// X=Up, Y=Right, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates URB;

            /// X=Up, Y=Back, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates UBR;

            /// X=Right, Y=Up, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RUB;

            /// X=Right, Y=Back, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RBU;

            /// X=Back, Y=Up, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BUR;

            /// X=Back, Y=Right, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BRU;

            /// X=Down, Y=Left, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DLF;

            /// X=Down, Y=Forward, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DFL;

            /// X=Left, Y=Down, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LDF;

            /// X=Left, Y=Forward, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LFD;

            /// X=Forward, Y=Down, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FDL;

            /// X=Forward, Y=Left, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FLD;

            /// X=Down, Y=Left, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DLB;

            /// X=Down, Y=Back, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DBL;

            /// X=Left, Y=Down, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LDB;

            /// X=Left, Y=Back, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LBD;

            /// X=Back, Y=Down, Z=Left
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BDL;

            /// X=Back, Y=Left, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BLD;

            /// X=Down, Y=Right, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DRF;

            /// X=Down, Y=Forward, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DFR;

            /// X=Right, Y=Down, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RDF;

            /// X=Right, Y=Forward, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RFD;

            /// X=Forward, Y=Down, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FDR;

            /// X=Forward, Y=Right, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates FRD;

            /// X=Down, Y=Right, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DRB;

            /// X=Down, Y=Back, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates DBR;

            /// X=Right, Y=Down, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RDB;

            /// X=Right, Y=Back, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RBD;

            /// X=Back, Y=Down, Z=Right
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BDR;

            /// X=Back, Y=Right, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates BRD;

            /// X=Up, Y=Right, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_X_UP;

            /// X=Down, Y=Right, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_X_DOWN;

            /// X=Right, Y=Up, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Y_UP;

            /// X=Right, Y=Down, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Y_DOWN;

            /// X=Right, Y=Forward, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Z_UP;

            /// X=Right, Y=Back, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates RIGHT_HAND_Z_DOWN;

            /// X=Up, Y=Right, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_X_UP;

            /// X=Down, Y=Right, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_X_DOWN;

            /// X=Right, Y=Up, Z=Forward
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Y_UP;

            /// X=Right, Y=Down, Z=Back
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Y_DOWN;

            /// X=Right, Y=Back, Z=Up
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Z_UP;

            /// X=Right, Y=Forward, Z=Down
            RERUN_SDK_EXPORT static const rerun::components::ViewCoordinates LEFT_HAND_Z_DOWN;

            // <END_GENERATED:declarations>

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
        // <BEGIN_GENERATED:definitions>
        // This section is generated by running `scripts/generate_view_coordinate_defs.py --cpp`
        const ViewCoordinates ViewCoordinates::ULF = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::UFL = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::LUF = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::LFU = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::FUL = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::FLU = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::ULB = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::UBL = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::LUB = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::LBU = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::BUL = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::BLU = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::URF = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::UFR = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::RUF = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::RFU = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::FUR = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::FRU = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::URB = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::UBR = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::RUB = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::RBU = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::BUR = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::BRU = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::DLF = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::DFL = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::LDF = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::LFD = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::FDL = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::FLD = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::DLB = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::DBL = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::LDB = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::LBD = ViewCoordinates(
            rerun::components::ViewCoordinates::Left, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::BDL = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Left
        );
        const ViewCoordinates ViewCoordinates::BLD = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Left,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::DRF = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::DFR = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::RDF = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::RFD = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::FDR = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::FRD = ViewCoordinates(
            rerun::components::ViewCoordinates::Forward, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::DRB = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::DBR = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::RDB = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::RBD = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::BDR = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Right
        );
        const ViewCoordinates ViewCoordinates::BRD = ViewCoordinates(
            rerun::components::ViewCoordinates::Back, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_X_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_X_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Y_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Y_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Z_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::RIGHT_HAND_Z_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Down
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_X_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Up, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_X_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Down, rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Y_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Up,
            rerun::components::ViewCoordinates::Forward
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Y_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Back
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Z_UP = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Back,
            rerun::components::ViewCoordinates::Up
        );
        const ViewCoordinates ViewCoordinates::LEFT_HAND_Z_DOWN = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Forward,
            rerun::components::ViewCoordinates::Down
        );
        // <END_GENERATED:definitions>

    } // namespace components
} // namespace rerun
