// Log an encoded depth image stored as a 16-bit PNG.

#include <rerun.hpp>

#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <utility>
#include <vector>

namespace fs = std::filesystem;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_encoded_depth_image");
    rec.spawn().exit_on_failure();

    const auto depth_path = fs::path(__FILE__).parent_path() / "encoded_depth.png";
    std::ifstream file(depth_path, std::ios::binary);
    if (!file) {
        std::cerr << "Failed to open encoded depth image: " << depth_path << std::endl;
        return 1;
    }

    std::vector<uint8_t> png_bytes(
        std::istreambuf_iterator<char>(file),
        std::istreambuf_iterator<char>()
    );

    const rerun::WidthHeight resolution(64, 48);
    const auto format =
        rerun::components::ImageFormat(resolution, rerun::datatypes::ChannelDatatype::U16);

    // Depth values are encoded as millimeters in the PNG payload.
    rec.log(
        "depth/encoded",
        rerun::archetypes::EncodedDepthImage()
            .with_blob(rerun::components::Blob(
                rerun::Collection<uint8_t>::take_ownership(std::move(png_bytes))
            ))
            .with_format(format)
            .with_media_type(rerun::components::MediaType::png())
            .with_meter(0.001f)
    );
}
