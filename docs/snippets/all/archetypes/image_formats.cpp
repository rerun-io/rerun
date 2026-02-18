#include <algorithm>
#include <cstdint>
#include <vector>

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_image_formats");
    rec.spawn().exit_on_failure();

    // Simple gradient image
    std::vector<uint8_t> image(256 * 256 * 3);
    for (size_t y = 0; y < 256; ++y) {
        for (size_t x = 0; x < 256; ++x) {
            image[(y * 256 + x) * 3 + 0] = static_cast<uint8_t>(x);
            image[(y * 256 + x) * 3 + 1] = static_cast<uint8_t>(std::min<size_t>(255, x + y));
            image[(y * 256 + x) * 3 + 2] = static_cast<uint8_t>(y);
        }
    }

    // RGB image
    rec.log("image_rgb", rerun::Image::from_rgb24(image, {256, 256}));

    // Green channel only (Luminance)
    std::vector<uint8_t> green_channel(256 * 256);
    for (size_t i = 0; i < 256 * 256; ++i) {
        green_channel[i] = image[i * 3 + 1];
    }
    rec.log(
        "image_green_only",
        rerun::Image(rerun::borrow(green_channel), {256, 256}, rerun::ColorModel::L)
    );

    // BGR image
    std::vector<uint8_t> bgr_image(256 * 256 * 3);
    for (size_t i = 0; i < 256 * 256; ++i) {
        bgr_image[i * 3 + 0] = image[i * 3 + 2];
        bgr_image[i * 3 + 1] = image[i * 3 + 1];
        bgr_image[i * 3 + 2] = image[i * 3 + 0];
    }
    rec.log(
        "image_bgr",
        rerun::Image(rerun::borrow(bgr_image), {256, 256}, rerun::ColorModel::BGR)
    );

    // New image with Separate Y/U/V planes with 4:2:2 chroma downsampling
    std::vector<uint8_t> yuv_bytes(256 * 256 + 128 * 256 * 2);
    std::fill_n(yuv_bytes.begin(), 256 * 256, static_cast<uint8_t>(128)); // Fixed value for Y
    size_t u_plane_offset = 256 * 256;
    size_t v_plane_offset = u_plane_offset + 128 * 256;
    for (size_t y = 0; y < 256; ++y) {
        for (size_t x = 0; x < 128; ++x) {
            auto coord = y * 128 + x;
            yuv_bytes[u_plane_offset + coord] = static_cast<uint8_t>(x * 2); // Gradient for U
            yuv_bytes[v_plane_offset + coord] = static_cast<uint8_t>(y);     // Gradient for V
        }
    }
    rec.log(
        "image_yuv422",
        rerun::Image(rerun::borrow(yuv_bytes), {256, 256}, rerun::PixelFormat::Y_U_V16_FullRange)
    );

    return 0;
}
