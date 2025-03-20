#include <rerun.hpp>

#include <algorithm> // std::max
#include <cmath>
#include <string>

using namespace std::chrono;

constexpr float TAU = 6.28318530717958647692528676655900577f;

void log_hand(
    const rerun::RecordingStream& rec, const char* name, seconds step, float angle, float length,
    float width, uint8_t blue
) {
    const auto tip = rerun::Vec3D{length * sinf(angle * TAU), length * cosf(angle * TAU), 0.0f};
    const auto c = static_cast<uint8_t>(angle * 255.0f);
    const auto color =
        rerun::Color{static_cast<uint8_t>(255 - c), c, blue, std::max<uint8_t>(128, blue)};

    rec.set_time_duration("sim_time", step);

    rec.log(
        std::string("world/") + name + "_pt",
        rerun::Points3D(rerun::Position3D(tip)).with_colors(color)
    );
    rec.log(
        std::string("world/") + name + "hand",
        rerun::Arrows3D::from_vectors(rerun::Vector3D(tip))
            .with_origins({{0.0f, 0.0f, 0.0f}})
            .with_colors(color)
            .with_radii({width * 0.5f})
    );
}

int main() {
    const float LENGTH_S = 20.0f;
    const float LENGTH_M = 10.0f;
    const float LENGTH_H = 4.0f;
    const float WIDTH_S = 0.25f;
    const float WIDTH_M = 0.4f;
    const float WIDTH_H = 0.6f;

    const int num_steps = 10000;

    const auto rec = rerun::RecordingStream("rerun_example_clock");
    rec.spawn().exit_on_failure();

    rec.log_static("world", rerun::ViewCoordinates::RIGHT_HAND_Y_UP);
    rec.log_static("world/frame", rerun::Boxes3D::from_half_sizes({{LENGTH_S, LENGTH_S, 1.0f}}));

    for (int step = 0; step < num_steps; step++) {
        log_hand(rec, "seconds", seconds(step), (step % 60) / 60.0f, LENGTH_S, WIDTH_S, 0);
        log_hand(rec, "minutes", seconds(step), (step % 3600) / 3600.0f, LENGTH_M, WIDTH_M, 128);
        log_hand(rec, "hours", seconds(step), (step % 43200) / 43200.0f, LENGTH_H, WIDTH_H, 255);
    }
}
