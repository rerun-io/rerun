#include <vector>

#include "benchmarks.hpp"
#include "profile_scope.hpp"

#include <rerun.hpp>

// About 1gb of image data.
constexpr size_t IMAGE_DIMENSION = 16384;
constexpr size_t IMAGE_CHANNELS = 4;

// How many times we log the image.
// Each time with a single pixel changed.
constexpr size_t NUM_LOG_CALLS = 4;

static std::vector<uint8_t> prepare() {
    PROFILE_FUNCTION();

    std::vector<uint8_t> image(
        IMAGE_DIMENSION * IMAGE_DIMENSION * IMAGE_CHANNELS,
        static_cast<uint8_t>(0)
    );

    return image;
}

static void execute(std::vector<uint8_t> raw_image_data) {
    PROFILE_FUNCTION();

    rerun::RecordingStream rec("rerun_example_benchmark_image");

    for (size_t i = 0; i < NUM_LOG_CALLS; ++i) {
        raw_image_data[i] += 1;
        rec.log(
            "test_image",
            rerun::Image(
                {
                    IMAGE_DIMENSION,
                    IMAGE_DIMENSION,
                    IMAGE_CHANNELS,
                },
                raw_image_data
            )
        );
    }
}

void run_image() {
    PROFILE_FUNCTION();
    auto input = prepare();
    execute(std::move(input));
}
