#include "quaternion.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        RERUN_SDK_EXPORT static const Quaternion IDENTITY;
        RERUN_SDK_EXPORT static const Quaternion INVALID;

        /// Construct Quaternion from x/y/z/w values.
        static Quaternion from_xyzw(float x, float y, float z, float w) {
            return Quaternion::from_xyzw({x, y, z, w});
        }

        /// Construct Quaternion from w/x/y/z values.
        static Quaternion from_wxyz(float w, float x, float y, float z) {
            return Quaternion::from_xyzw(x, y, z, w);
        }

        /// Construct Quaternion from x/y/z/w array.
        static Quaternion from_xyzw(std::array<float, 4> xyzw_) {
            Quaternion q;
            q.xyzw = xyzw_;
            return q;
        }

        /// Construct Quaternion from w/x/y/z array.
        static Quaternion from_wxyz(std::array<float, 4> wxyz_) {
            return Quaternion::from_xyzw(wxyz_[1], wxyz_[2], wxyz_[3], wxyz_[0]);
        }

        /// Construct Quaternion from x/y/z/w float pointer.
        static Quaternion from_xyzw(const float* xyzw_) {
            return Quaternion::from_xyzw(xyzw_[0], xyzw_[1], xyzw_[2], xyzw_[3]);
        }

        /// Construct Quaternion from w/x/y/z float pointer.
        static Quaternion from_wxyz(const float* wxyz_) {
            return Quaternion::from_xyzw(wxyz_[1], wxyz_[2], wxyz_[3], wxyz_[0]);
        }

        float x() const {
            return xyzw[0];
        }

        float y() const {
            return xyzw[1];
        }

        float z() const {
            return xyzw[2];
        }

        float w() const {
            return xyzw[3];
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

        const Quaternion Quaternion::IDENTITY = Quaternion::from_xyzw(0.0f, 0.0f, 0.0f, 1.0f);
        const Quaternion Quaternion::INVALID = Quaternion::from_xyzw(0.0f, 0.0f, 0.0f, 0.0f);
    } // namespace datatypes
} // namespace rerun
