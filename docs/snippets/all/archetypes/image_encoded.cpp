// Create and log a image.

#include <rerun.hpp>

#include <filesystem>
#include <fstream>
#include <iostream>
#include <vector>

namespace fs = std::filesystem;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_image_encoded");
    rec.spawn().exit_on_failure();

    fs::path image_filepath = fs::path(__FILE__).parent_path() / "ferris.png";

    rec.log("image", rerun::ImageEncoded::from_file(image_filepath).value_or_throw());
}
