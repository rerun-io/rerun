// Create and log a image.

#include <rerun.hpp>

#include <filesystem>
#include <fstream>
#include <iostream>
#include <vector>

namespace fs = std::filesystem;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_encoded_image");
    rec.spawn().exit_on_failure();

    fs::path image_filepath = fs::path(__FILE__).parent_path() / "ferris.png";

    rec.log("image", rerun::EncodedImage::from_file(image_filepath).value_or_throw());
}
