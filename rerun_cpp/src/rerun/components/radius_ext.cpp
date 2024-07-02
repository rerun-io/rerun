//#define EDIT_EXTENSION
#ifdef EDIT_EXTENSION

#include "radius.hpp"

// Uncomment for better auto-complete while editing the extension.

namespace rerun {
    namespace components {
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new radius in scene units.
        ///
        /// Values passed must be finite positive.
        static Radius scene_units(float radius_in_scene_units) {
            return Radius(radius_in_scene_units);
        }

        /// Creates a new radius in ui points.
        ///
        /// Values passed must be finite positive.
        static Radius ui_points(float radius_in_ui_points) {
            return Radius(-radius_in_ui_points);
        }

        // </CODEGEN_COPY_TO_HEADER>
    } // namespace components
} // namespace rerun

#endif
