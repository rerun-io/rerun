// Log a few gaussian splats.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_gaussian_splats3d");
    rec.spawn().exit_on_failure();

    // 15 view-dependent RGB coefficients (degrees 1-3) per splat, coefficient-major:
    std::array<std::array<float, 3>, 15> red_sh, green_sh, blue_sh;
    red_sh.fill({0.5f, 0.0f, 0.0f});
    green_sh.fill({0.0f, 0.5f, 0.0f});
    blue_sh.fill({0.0f, 0.0f, 0.5f});

    rec.log(
        "gaussians",
        rerun::GaussianSplats3D(
            {{0.0f, 0.0f, 0.0f}, {2.0f, 0.0f, 0.0f}, {4.0f, 0.0f, 0.0f}}
        )
            .with_scales(
                {{1.0f, 0.5f, 0.25f}, {0.5f, 1.0f, 0.5f}, {0.25f, 0.5f, 1.0f}}
            )
            .with_quaternions({
                rerun::Quaternion::IDENTITY,
                // 45 degrees around Z
                rerun::Quaternion::from_xyzw(0.0f, 0.0f, 0.382683f, 0.923880f),
                rerun::Quaternion::IDENTITY,
            })
            .with_colors({
                rerun::Rgba32(255, 0, 0, 128),
                rerun::Rgba32(0, 255, 0, 200),
                rerun::Rgba32(0, 0, 255, 255),
            })
            .with_sh_coefficients({
                rerun::datatypes::SphericalHarmonics3Rgb(red_sh),
                rerun::datatypes::SphericalHarmonics3Rgb(green_sh),
                rerun::datatypes::SphericalHarmonics3Rgb(blue_sh),
            })
    );
}
