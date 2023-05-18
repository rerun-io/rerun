"""Test for color_conversion module."""
import numpy as np
from depthai_viewer.color_conversion import linear_to_gamma_u8_pixel, linear_to_gamma_u8_value


def test_linear_to_gamma_u8_value() -> None:
    input = np.array((-0.1, 0, 0.003, 0.0031308, 0.5, 1.0, 1.001))
    expected = np.array((0, 0, 10, 10, 188, 255, 255), dtype=np.uint8)
    actual = linear_to_gamma_u8_value(linear=input)
    np.testing.assert_array_equal(actual, expected)


def test_linear_to_gamma_u8_pixel_rgb() -> None:
    input = np.array(((0.003, 0.003, 0.003), (0.5, 0.5, 0.5)))
    expected = np.array(((10, 10, 10), (188, 188, 188)))
    actual = linear_to_gamma_u8_pixel(linear=input)
    np.testing.assert_array_equal(actual, expected)


def test_linear_to_gamma_u8_pixel_rgba() -> None:
    input = np.array(((0.003, 0.003, 0.003, 0.003), (0.5, 0.5, 0.5, 0.5)))
    expected = np.array(((10, 10, 10, 1), (188, 188, 188, 128)))
    actual = linear_to_gamma_u8_pixel(linear=input)
    np.testing.assert_array_equal(actual, expected)
