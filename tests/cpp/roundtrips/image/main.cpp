// Logs an `Image` archetype for roundtrip checks.

#include <rerun/archetypes/image.hpp>

#include <rerun/recording_stream.hpp>

uint32_t as_uint(float f) {
    // Don't do `*reinterpret_cast<const uint32_t*>(&x)` since it breaks strict aliasing rules.
    uint32_t n;
    memcpy(&n, &f, sizeof(float));
    return n;
}

// Adopted from https://stackoverflow.com/a/60047308
// IEEE-754 16-bit floating-point format (without infinity): 1-5-10, exp-15, +-131008.0, +-6.1035156E-5, +-5.9604645E-8, 3.311 digits
rerun::half half_from_float(const float x) {
    // round-to-nearest-even: add last bit after truncated mantissa1
    const uint32_t b = as_uint(x) + 0x00001000;
    const uint32_t e = (b & 0x7F800000) >> 23; // exponent
    // mantissa; in line below: 0x007FF000 = 0x00800000-0x00001000 = decimal indicator flag - initial rounding
    const uint32_t m = b & 0x007FFFFF;
    const uint32_t f16 = (b & 0x80000000) >> 16 |
                         (e > 112) * ((((e - 112) << 10) & 0x7C00) | m >> 13) |
                         ((e < 113) & (e > 101)) * ((((0x007FF000 + m) >> (125 - e)) + 1) >> 1) |
                         (e > 143) * 0x7FFF; // sign : normalized : denormalized : saturate
    return rerun::half{static_cast<uint16_t>(f16)};
}

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_image");
    rec.save(argv[1]).exit_on_failure();

    // h=2 w=3 c=3 image. Red channel = x. Green channel = y. Blue channel = 128.
    {
        auto img = rerun::datatypes::TensorData(
            {2, 3, 3},
            std::vector<uint8_t>{0, 0, 128, 1, 0, 128, 2, 0, 128, 0, 1, 128, 1, 1, 128, 2, 1, 128}
        );
        rec.log("image", rerun::archetypes::Image(img));
    }

    // h=4, w=5 mono image. Pixel = x * y * 123.4
    {
        std::vector<rerun::half> data;
        for (auto y = 0; y < 4; ++y) {
            for (auto x = 0; x < 5; ++x) {
                data.push_back(half_from_float(static_cast<float>(x * y) * 123.4f));
            }
        }
        auto img = rerun::datatypes::TensorData({4, 5}, std::move(data));
        rec.log("image_f16", rerun::archetypes::Image(img));
    }
}
