#include "pinhole.hpp"

namespace rerun {
    namespace archetypes {
#ifdef CODEGEN
        // [CODEGEN COPY TO HEADER START]

        /// Creates a pinhole from the camera focal length and resolution, both specified in pixels.
        ///
        /// The focal length is the diagonal of the projection matrix.
        /// Set the same value for x & y value for symmetric cameras, or two values for anamorphic
        /// cameras.
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole focal_length_and_resolution(
            const datatypes::Vec2D& focal_length, const datatypes::Vec2D& resolution
        );

        // [CODEGEN COPY TO HEADER END]
#endif

        Pinhole Pinhole::focal_length_and_resolution(
            const datatypes::Vec2D& focal_length, const datatypes::Vec2D& _resolution
        ) {
            const float u_cen = _resolution.x() / 2.0f;
            const float v_cen = _resolution.y() / 2.0f;

            auto pinhole = Pinhole(datatypes::Mat3x3(
                {{focal_length.x(), 0.0f, 0.0f},
                 {0.0f, focal_length.y(), 0.0f},
                 {u_cen, v_cen, 1.0f}}
            ));
            pinhole.resolution = _resolution;
            return pinhole;
        }

    } // namespace archetypes
} // namespace rerun
