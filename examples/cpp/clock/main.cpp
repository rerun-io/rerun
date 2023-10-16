#include <algorithm>
#include <cmath>
#include <rerun.hpp>

const float TAU = static_cast<float>(2.0 * M_PI);

void log_hand(
    rerun::RecordingStream& rec, const char* name, int step, float angle, float length, float width,
    uint8_t blue
) {
    rerun::datatypes::Vec3D tip{length * sinf(angle * TAU), length * cosf(angle * TAU), 0.0f};
    uint8_t c = static_cast<uint8_t>(angle * 255.0f);
    rerun::components::Color color{
        static_cast<uint8_t>(255 - c),
        c,
        blue,
        std::max<uint8_t>(128, blue)
    };

    rec.set_time_seconds("sim_time", step);

    rec.log(
        (std::string("world/") + name + "_pt").c_str(),
        rerun::Points3D(rerun::components::Position3D(tip)).with_colors(color)
    );
    rec.log(
        (std::string("world/") + name + "hand").c_str(),
        rerun::Arrows3D::from_vectors(rerun::components::Vector3D(tip))
            .with_origins({{0.0f, 0.0f, 0.0f}})
            .with_colors(color)
            .with_radii({width * 0.5f})
    );
}

int main(int argc, char** argv) {
    const float LENGTH_S = 20.0f;
    const float LENGTH_M = 10.0f;
    const float LENGTH_H = 4.0f;
    const float WIDTH_S = 0.25f;
    const float WIDTH_M = 0.4f;
    const float WIDTH_H = 0.6f;

    const int num_steps = 10000;

    auto rec = rerun::RecordingStream("rerun_example_clock");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log_timeless("world", rerun::ViewCoordinates::RIGHT_HAND_Y_UP);
    rec.log_timeless("world/frame", rerun::Boxes3D::from_half_sizes({{LENGTH_S, LENGTH_S, 1.0f}}));

    for (int step = 0; step < num_steps; step++) {
        log_hand(rec, "seconds", step, (step % 60) / 60.0f, LENGTH_S, WIDTH_S, 0);
        log_hand(rec, "minutes", step, (step % 3600) / 3600.0f, LENGTH_M, WIDTH_M, 128);
        log_hand(rec, "hours", step, (step % 43200) / 43200.0f, LENGTH_H, WIDTH_H, 255);
    };
}
