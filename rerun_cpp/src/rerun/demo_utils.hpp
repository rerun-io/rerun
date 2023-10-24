// Utilities used in examples.

#include <algorithm>
#include <cmath>
#include <vector>

#include "components/color.hpp"
#include "components/position3d.hpp"

namespace rerun {
    namespace demo {
        constexpr float TAU = static_cast<float>(M_PI * 2.0);

        /// A linear interpolator that bounces between `a` and `b` as `t` goes above `1.0`.
        inline float bounce_lerp(float a, float b, float t) {
            auto tf = t - static_cast<int32_t>(t);
            if (static_cast<int32_t>(t) % 2 == 0) {
                return (1.0f - tf) * a + tf * b;
            } else {
                return tf * a + (1.0f - tf) * b;
            }
        }

        /// Linearly interpolates from `a` through `b` in `n` steps, returning the intermediate result at
        /// each step.
        template <typename T>
        inline std::vector<T> linspace(T start, T end, size_t num) {
            std::vector<T> linspaced(num);
            std::generate(linspaced.begin(), linspaced.end(), [&, i = 0]() mutable {
                return start + i++ * (end - start) / static_cast<T>(num - 1);
            });
            return linspaced;
        }

        /// Given two 3D vectors `from` and `to`, linearly interpolates between them in `n` steps along
        /// the three axes, returning the intermediate result at each step.
        std::vector<components::Position3D> grid(
            components::Position3D from, components::Position3D to, size_t n
        );

        /// Create a spiral of points with colors along the Z axis.
        ///
        /// * `num_points`: Total number of points.
        /// * `radius`: The radius of the spiral.
        /// * `angular_step`: The factor applied between each step along the trigonometric circle.
        /// * `angular_offset`: Offsets the starting position on the trigonometric circle.
        /// * `z_step`: The factor applied between between each step along the Z axis.
        void color_spiral(
            size_t num_points, float radius, float angular_step, float angular_offset, float z_step,
            std::vector<components::Position3D>& out_points,
            std::vector<components::Color>& out_colors
        );

        /// Returns sRGB polynomial approximation from Turbo color map, assuming `t` is normalized.
        rerun::components::Color colormap_turbo_srgb(float t);
    } // namespace demo
} // namespace rerun
