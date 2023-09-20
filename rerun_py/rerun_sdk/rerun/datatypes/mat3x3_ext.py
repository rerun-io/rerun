from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from . import Mat3x3Like


class Mat3x3Ext:
    @staticmethod
    def flat_columns__field_converter_override(data: Mat3x3Like) -> npt.NDArray[np.float32]:
        from . import Mat3x3

        if isinstance(data, Mat3x3):
            return data.flat_columns
        else:
            arr = np.array(data, dtype=np.float32).reshape(3, 3)
            return arr.flatten("F")
