# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import numpy as np

from .rerun_sdk import *

print("rerun_sdk initialized")


def log_points(name, positions, colors):
    if colors is not None:
        # Rust expects colors in 0-255 uint8
        if colors.dtype in ['float32', 'float64']:
            max = np.amax(colors)
            if max < 1.1:
                # TODO: gamma curve correction for RGB, and just *255 for alpgha
                raise TypeError(
                    "Expected color values in 0-255 gamma range, but got color values in 0-1 range")

        colors = colors.astype('uint8')
        # TODO: extend colors with alpha=255 if colors is Nx3

    positions.astype('float32')

    log_points_rs(name, positions, colors)
