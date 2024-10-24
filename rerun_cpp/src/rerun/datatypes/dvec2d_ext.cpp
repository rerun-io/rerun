#include "dvec2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct DVec2DExt {
            double xy[2];
#define DVec2D DVec2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct DVec2D from x/y values.
            DVec2D(double x, double y) : xy{x, y} {}

            /// Construct DVec2D from x/y double pointer.
            explicit DVec2D(const double* xy_) : xy{xy_[0], xy_[1]} {}

            double x() const {
                return xy[0];
            }

            double y() const {
                return xy[1];
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
