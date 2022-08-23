"""Test for color_conversion module."""
import numpy as np

from rerun_sdk.color_converstion import linear_to_gamma_u8


def test_linear_to_gamma_u8():
    input = np.array((-0.1, 0, 0.003, 0.5, 1.0, 1.001))
    expected = np.array((0, 0, 10, 188, 255, 255), dtype=np.uint8)
    actual = linear_to_gamma_u8(linear=input)
    np.testing.assert_array_equal(actual, expected)
