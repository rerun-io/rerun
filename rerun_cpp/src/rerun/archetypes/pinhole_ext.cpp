#include "pinhole.hpp"

namespace rerun {
    namespace archetypes {
#ifdef CODEGEN
        // <CODEGEN_COPY_TO_HEADER>

#include <cmath>
#include <limits>

        /// Creates a pinhole from the camera focal length and resolution, both specified in pixels.
        ///
        /// The focal length is the diagonal of the projection matrix.
        /// Set the same value for x & y value for symmetric cameras, or two values for anamorphic
        /// cameras.
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_focal_length_and_resolution(
            const datatypes::Vec2D& focal_length, const datatypes::Vec2D& resolution
        );

        /// Creates a symmetric pinhole from the camera focal length and resolution, both specified
        /// in pixels.
        ///
        /// The focal length is the diagonal of the projection matrix.
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_focal_length_and_resolution(
            float focal_length, const datatypes::Vec2D& resolution
        ) {
            return from_focal_length_and_resolution({focal_length, focal_length}, resolution);
        }

        /// Creates a pinhole from the camera vertical field of view (in radians) and aspect ratio (width/height).
        ///
        /// Assumes the principal point to be in the middle of the sensor.
        static Pinhole from_fov_and_aspect_ratio(float fov_y, float aspect_ratio) {
            const float EPSILON = std::numeric_limits<float>::epsilon();
            const float focal_length_y = 0.5f / std::tan(std::max(fov_y * 0.5f, EPSILON));
            return from_focal_length_and_resolution(
                {focal_length_y, focal_length_y},
                {aspect_ratio, 1.0}
            );
        }

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        Pinhole with_resolution(float width, float height) && {
            resolution = rerun::components::Resolution(width, height);
            return std::move(*this);
        }

        /// Pixel resolution (usually integers) of child image space. Width and height.
        ///
        /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        Pinhole with_resolution(int width, int height) && {
            resolution = rerun::components::Resolution(width, height);
            return std::move(*this);
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

        Pinhole Pinhole::from_focal_length_and_resolution(
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
