#include <catch2/catch_test_macros.hpp>

#include <cmath>
#include <cstdint>

#include <rerun/half.hpp>

#define TEST_TAG "[half]"

// Golden bit patterns produced by numpy: `np.float32(v).astype(np.float16).view(np.uint16)`.
TEST_CASE("half::from_float rounds to nearest, ties to even", TEST_TAG) {
    SECTION("Exactly representable values") {
        CHECK(rerun::half::from_float(0.0f).f16 == 0x0000);
        CHECK(rerun::half::from_float(-0.0f).f16 == 0x8000);
        CHECK(rerun::half::from_float(1.0f).f16 == 0x3c00);
        CHECK(rerun::half::from_float(-1.0f).f16 == 0xbc00);
        CHECK(rerun::half::from_float(0.5f).f16 == 0x3800);
        CHECK(rerun::half::from_float(0.25f).f16 == 0x3400);
        CHECK(rerun::half::from_float(2.0f).f16 == 0x4000);
        CHECK(rerun::half::from_float(-2.0f).f16 == 0xc000);
    }

    SECTION("Extremes of the normal range") {
        CHECK(rerun::half::from_float(65504.0f).f16 == 0x7bff);         // largest finite half
        CHECK(rerun::half::from_float(6.103515625e-05f).f16 == 0x0400); // smallest normal (2^-14)
    }

    SECTION("Subnormals and underflow") {
        CHECK(rerun::half::from_float(5.9604645e-08f).f16 == 0x0001); // smallest subnormal (2^-24)
        CHECK(rerun::half::from_float(2.9802322e-08f).f16 == 0x0000); // 2^-25: ties to even -> zero
    }

    SECTION("Rounding, ties to even") {
        CHECK(rerun::half::from_float(3.14159265f).f16 == 0x4248);
        // Halfway between 1.0 (even) and 1.0009766: rounds down to the even neighbor.
        CHECK(rerun::half::from_float(1.0f + 0.00048828125f).f16 == 0x3c00);
        // Halfway between 1.0009766 (odd) and 1.0019531 (even): rounds up to the even neighbor.
        CHECK(rerun::half::from_float(1.0f + 0.00146484375f).f16 == 0x3c02);
    }

    SECTION("Overflow to infinity") {
        CHECK(rerun::half::from_float(65536.0f).f16 == 0x7c00);
        CHECK(rerun::half::from_float(1e30f).f16 == 0x7c00);
        CHECK(rerun::half::from_float(-1e30f).f16 == 0xfc00);
    }

    SECTION("Infinity and NaN") {
        CHECK(rerun::half::from_float(INFINITY).f16 == 0x7c00);
        CHECK(rerun::half::from_float(-INFINITY).f16 == 0xfc00);
        const uint16_t nan_bits = rerun::half::from_float(NAN).f16;
        CHECK((nan_bits & 0x7c00) == 0x7c00); // exponent all ones
        CHECK((nan_bits & 0x03ff) != 0);      // non-zero mantissa
    }
}
