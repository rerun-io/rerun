from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from . import Mat4x4Like


class Mat4x4Ext:
    @staticmethod
    def coeffs__field_converter_override(data: Mat4x4Like) -> npt.NDArray[np.float32]:
        from . import Mat4x4

        if isinstance(data, Mat4x4):
            return data.coeffs
        else:
            arr = np.array(data, dtype=np.float32).reshape(4, 4)
            return arr.flatten("F")
