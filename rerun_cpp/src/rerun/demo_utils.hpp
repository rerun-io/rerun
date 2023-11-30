#pragma once

// Utilities used in examples.

#include <algorithm>
#include <cmath>
#include <vector>

#include "components/color.hpp"
#include "components/position3d.hpp"

namespace rerun {
    namespace demo {
        constexpr float PI = 3.14159265358979323846264338327950288f;
        constexpr float TAU = 6.28318530717958647692528676655900577f;

        /// A linear interpolator that bounces between `a` and `b` as `t` goes above `1.0`.
        inline float bounce_lerp(float a, float b, float t) {
            auto tf = t - floorf(t);
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
                return static_cast<T>(
                    start + static_cast<T>(i++) * (end - start) / static_cast<T>(num - 1)
                );
            });
            return linspaced;
        }

        /// Given a range `from`-`to`, linearly interpolates between them in `n` steps along
        /// three axes each, returning the intermediate result at each step.
        template <typename T, typename Elem>
        std::vector<T> grid3d(Elem from, Elem to, size_t n) {
            std::vector<T> output;
            for (Elem z : linspace(from, to, n)) {
                for (Elem y : linspace(from, to, n)) {
                    for (Elem x : linspace(from, to, n)) {
                        output.emplace_back(
                            static_cast<Elem>(x),
                            static_cast<Elem>(y),
                            static_cast<Elem>(z)
                        );
                    }
                }
            }
            return output;
        }

        /// Create a spiral of points with colors along the Z axis.
        ///
        /// * `num_points`: Total number of points.
        /// * `radius`: The radius of the spiral.
        /// * `angular_step`: The factor applied between each step along the trigonometric circle.
        /// * `angular_offset`: Offsets the starting position on the trigonometric circle.
        /// * `z_step`: The factor applied between each step along the Z axis.
        void color_spiral(
            size_t num_points, float radius, float angular_step, float angular_offset, float z_step,
            std::vector<components::Position3D>& out_points,
            std::vector<components::Color>& out_colors
        );

        /// Returns sRGB polynomial approximation from Turbo color map, assuming `t` is normalized.
        rerun::components::Color colormap_turbo_srgb(float t);
    } // namespace demo
} // namespace rerun
