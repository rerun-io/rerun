from __future__ import annotations

from enum import IntEnum
from typing import TYPE_CHECKING, Any, Iterable, cast

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from .._log import ComponentBatchLike
    from . import ViewCoordinates, ViewCoordinatesArrayLike


class ViewCoordinatesExt:
    class ViewDir(IntEnum):
        Up = 1
        Down = 2
        Right = 3
        Left = 4
        Forward = 5
        Back = 6

    @staticmethod
    def coordinates__field_converter_override(data: npt.ArrayLike) -> npt.NDArray[np.uint8]:
        coordinates = np.asarray(data, dtype=np.uint8)
        if coordinates.shape != (3,):
            raise ValueError(f"ViewCoordinates must be a 3-element array. Got: {coordinates.shape}")
        return coordinates

    @staticmethod
    def native_to_pa_array_override(data: ViewCoordinatesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ViewCoordinates

        if isinstance(data, ViewCoordinates):
            # ViewCoordinates
            data = [data.coordinates]
        elif hasattr(data, "__len__") and len(data) > 0 and isinstance(data[0], ViewCoordinates):  # type: ignore[arg-type, index]
            # [ViewCoordinates]
            data = [d.coordinates for d in data]  # type: ignore[union-attr]
        else:
            try:
                # [x, y, z]
                data = [ViewCoordinates(data).coordinates]
            except ValueError:
                # [[x, y, z], ...]
                data = [ViewCoordinates(d).coordinates for d in data]  # type: ignore[union-attr]

        data = np.asarray(data, dtype=np.uint8)

        if len(data.shape) != 2 or data.shape[1] != 3:
            raise ValueError(f"ViewCoordinates must be a 3-element array. Got: {data.shape}")

        data = data.flatten()

        for value in data:
            # TODO(jleibs): Enforce this validation based on ViewDir
            if value not in range(1, 7):
                raise ValueError("ViewCoordinates must contain only values in the range [1,6].")

        return pa.FixedSizeListArray.from_arrays(data, type=data_type)

    # Implement the AsComponents protocol
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..archetypes import ViewCoordinates
        from ..components import ViewCoordinates as ViewCoordinatesComponent

        return ViewCoordinates(cast(ViewCoordinatesComponent, self)).as_component_batches()

    def num_instances(self) -> int:
        # Always a mono-component
        return 1

    # <BEGIN_GENERATED:declarations>
    # This section is generated by running `scripts/generate_view_coordinate_defs.py --python`
    # The following declarations are replaced in `deferred_patch_class`.
    ULF: ViewCoordinates = None  # type: ignore[assignment]
    UFL: ViewCoordinates = None  # type: ignore[assignment]
    LUF: ViewCoordinates = None  # type: ignore[assignment]
    LFU: ViewCoordinates = None  # type: ignore[assignment]
    FUL: ViewCoordinates = None  # type: ignore[assignment]
    FLU: ViewCoordinates = None  # type: ignore[assignment]
    ULB: ViewCoordinates = None  # type: ignore[assignment]
    UBL: ViewCoordinates = None  # type: ignore[assignment]
    LUB: ViewCoordinates = None  # type: ignore[assignment]
    LBU: ViewCoordinates = None  # type: ignore[assignment]
    BUL: ViewCoordinates = None  # type: ignore[assignment]
    BLU: ViewCoordinates = None  # type: ignore[assignment]
    URF: ViewCoordinates = None  # type: ignore[assignment]
    UFR: ViewCoordinates = None  # type: ignore[assignment]
    RUF: ViewCoordinates = None  # type: ignore[assignment]
    RFU: ViewCoordinates = None  # type: ignore[assignment]
    FUR: ViewCoordinates = None  # type: ignore[assignment]
    FRU: ViewCoordinates = None  # type: ignore[assignment]
    URB: ViewCoordinates = None  # type: ignore[assignment]
    UBR: ViewCoordinates = None  # type: ignore[assignment]
    RUB: ViewCoordinates = None  # type: ignore[assignment]
    RBU: ViewCoordinates = None  # type: ignore[assignment]
    BUR: ViewCoordinates = None  # type: ignore[assignment]
    BRU: ViewCoordinates = None  # type: ignore[assignment]
    DLF: ViewCoordinates = None  # type: ignore[assignment]
    DFL: ViewCoordinates = None  # type: ignore[assignment]
    LDF: ViewCoordinates = None  # type: ignore[assignment]
    LFD: ViewCoordinates = None  # type: ignore[assignment]
    FDL: ViewCoordinates = None  # type: ignore[assignment]
    FLD: ViewCoordinates = None  # type: ignore[assignment]
    DLB: ViewCoordinates = None  # type: ignore[assignment]
    DBL: ViewCoordinates = None  # type: ignore[assignment]
    LDB: ViewCoordinates = None  # type: ignore[assignment]
    LBD: ViewCoordinates = None  # type: ignore[assignment]
    BDL: ViewCoordinates = None  # type: ignore[assignment]
    BLD: ViewCoordinates = None  # type: ignore[assignment]
    DRF: ViewCoordinates = None  # type: ignore[assignment]
    DFR: ViewCoordinates = None  # type: ignore[assignment]
    RDF: ViewCoordinates = None  # type: ignore[assignment]
    RFD: ViewCoordinates = None  # type: ignore[assignment]
    FDR: ViewCoordinates = None  # type: ignore[assignment]
    FRD: ViewCoordinates = None  # type: ignore[assignment]
    DRB: ViewCoordinates = None  # type: ignore[assignment]
    DBR: ViewCoordinates = None  # type: ignore[assignment]
    RDB: ViewCoordinates = None  # type: ignore[assignment]
    RBD: ViewCoordinates = None  # type: ignore[assignment]
    BDR: ViewCoordinates = None  # type: ignore[assignment]
    BRD: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_X_UP: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_X_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_Y_UP: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_Y_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_Z_UP: ViewCoordinates = None  # type: ignore[assignment]
    RIGHT_HAND_Z_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_X_UP: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_X_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_Y_UP: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_Y_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_Z_UP: ViewCoordinates = None  # type: ignore[assignment]
    LEFT_HAND_Z_DOWN: ViewCoordinates = None  # type: ignore[assignment]
    # <END_GENERATED:declarations>

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        # <BEGIN_GENERATED:definitions>
        # This section is generated by running `scripts/generate_view_coordinate_defs.py --python`
        cls.ULF = cls([cls.ViewDir.Up, cls.ViewDir.Left, cls.ViewDir.Forward])
        cls.UFL = cls([cls.ViewDir.Up, cls.ViewDir.Forward, cls.ViewDir.Left])
        cls.LUF = cls([cls.ViewDir.Left, cls.ViewDir.Up, cls.ViewDir.Forward])
        cls.LFU = cls([cls.ViewDir.Left, cls.ViewDir.Forward, cls.ViewDir.Up])
        cls.FUL = cls([cls.ViewDir.Forward, cls.ViewDir.Up, cls.ViewDir.Left])
        cls.FLU = cls([cls.ViewDir.Forward, cls.ViewDir.Left, cls.ViewDir.Up])
        cls.ULB = cls([cls.ViewDir.Up, cls.ViewDir.Left, cls.ViewDir.Back])
        cls.UBL = cls([cls.ViewDir.Up, cls.ViewDir.Back, cls.ViewDir.Left])
        cls.LUB = cls([cls.ViewDir.Left, cls.ViewDir.Up, cls.ViewDir.Back])
        cls.LBU = cls([cls.ViewDir.Left, cls.ViewDir.Back, cls.ViewDir.Up])
        cls.BUL = cls([cls.ViewDir.Back, cls.ViewDir.Up, cls.ViewDir.Left])
        cls.BLU = cls([cls.ViewDir.Back, cls.ViewDir.Left, cls.ViewDir.Up])
        cls.URF = cls([cls.ViewDir.Up, cls.ViewDir.Right, cls.ViewDir.Forward])
        cls.UFR = cls([cls.ViewDir.Up, cls.ViewDir.Forward, cls.ViewDir.Right])
        cls.RUF = cls([cls.ViewDir.Right, cls.ViewDir.Up, cls.ViewDir.Forward])
        cls.RFU = cls([cls.ViewDir.Right, cls.ViewDir.Forward, cls.ViewDir.Up])
        cls.FUR = cls([cls.ViewDir.Forward, cls.ViewDir.Up, cls.ViewDir.Right])
        cls.FRU = cls([cls.ViewDir.Forward, cls.ViewDir.Right, cls.ViewDir.Up])
        cls.URB = cls([cls.ViewDir.Up, cls.ViewDir.Right, cls.ViewDir.Back])
        cls.UBR = cls([cls.ViewDir.Up, cls.ViewDir.Back, cls.ViewDir.Right])
        cls.RUB = cls([cls.ViewDir.Right, cls.ViewDir.Up, cls.ViewDir.Back])
        cls.RBU = cls([cls.ViewDir.Right, cls.ViewDir.Back, cls.ViewDir.Up])
        cls.BUR = cls([cls.ViewDir.Back, cls.ViewDir.Up, cls.ViewDir.Right])
        cls.BRU = cls([cls.ViewDir.Back, cls.ViewDir.Right, cls.ViewDir.Up])
        cls.DLF = cls([cls.ViewDir.Down, cls.ViewDir.Left, cls.ViewDir.Forward])
        cls.DFL = cls([cls.ViewDir.Down, cls.ViewDir.Forward, cls.ViewDir.Left])
        cls.LDF = cls([cls.ViewDir.Left, cls.ViewDir.Down, cls.ViewDir.Forward])
        cls.LFD = cls([cls.ViewDir.Left, cls.ViewDir.Forward, cls.ViewDir.Down])
        cls.FDL = cls([cls.ViewDir.Forward, cls.ViewDir.Down, cls.ViewDir.Left])
        cls.FLD = cls([cls.ViewDir.Forward, cls.ViewDir.Left, cls.ViewDir.Down])
        cls.DLB = cls([cls.ViewDir.Down, cls.ViewDir.Left, cls.ViewDir.Back])
        cls.DBL = cls([cls.ViewDir.Down, cls.ViewDir.Back, cls.ViewDir.Left])
        cls.LDB = cls([cls.ViewDir.Left, cls.ViewDir.Down, cls.ViewDir.Back])
        cls.LBD = cls([cls.ViewDir.Left, cls.ViewDir.Back, cls.ViewDir.Down])
        cls.BDL = cls([cls.ViewDir.Back, cls.ViewDir.Down, cls.ViewDir.Left])
        cls.BLD = cls([cls.ViewDir.Back, cls.ViewDir.Left, cls.ViewDir.Down])
        cls.DRF = cls([cls.ViewDir.Down, cls.ViewDir.Right, cls.ViewDir.Forward])
        cls.DFR = cls([cls.ViewDir.Down, cls.ViewDir.Forward, cls.ViewDir.Right])
        cls.RDF = cls([cls.ViewDir.Right, cls.ViewDir.Down, cls.ViewDir.Forward])
        cls.RFD = cls([cls.ViewDir.Right, cls.ViewDir.Forward, cls.ViewDir.Down])
        cls.FDR = cls([cls.ViewDir.Forward, cls.ViewDir.Down, cls.ViewDir.Right])
        cls.FRD = cls([cls.ViewDir.Forward, cls.ViewDir.Right, cls.ViewDir.Down])
        cls.DRB = cls([cls.ViewDir.Down, cls.ViewDir.Right, cls.ViewDir.Back])
        cls.DBR = cls([cls.ViewDir.Down, cls.ViewDir.Back, cls.ViewDir.Right])
        cls.RDB = cls([cls.ViewDir.Right, cls.ViewDir.Down, cls.ViewDir.Back])
        cls.RBD = cls([cls.ViewDir.Right, cls.ViewDir.Back, cls.ViewDir.Down])
        cls.BDR = cls([cls.ViewDir.Back, cls.ViewDir.Down, cls.ViewDir.Right])
        cls.BRD = cls([cls.ViewDir.Back, cls.ViewDir.Right, cls.ViewDir.Down])
        cls.RIGHT_HAND_X_UP = cls([cls.ViewDir.Up, cls.ViewDir.Right, cls.ViewDir.Forward])
        cls.RIGHT_HAND_X_DOWN = cls([cls.ViewDir.Down, cls.ViewDir.Right, cls.ViewDir.Back])
        cls.RIGHT_HAND_Y_UP = cls([cls.ViewDir.Right, cls.ViewDir.Up, cls.ViewDir.Back])
        cls.RIGHT_HAND_Y_DOWN = cls([cls.ViewDir.Right, cls.ViewDir.Down, cls.ViewDir.Forward])
        cls.RIGHT_HAND_Z_UP = cls([cls.ViewDir.Right, cls.ViewDir.Forward, cls.ViewDir.Up])
        cls.RIGHT_HAND_Z_DOWN = cls([cls.ViewDir.Right, cls.ViewDir.Back, cls.ViewDir.Down])
        cls.LEFT_HAND_X_UP = cls([cls.ViewDir.Up, cls.ViewDir.Right, cls.ViewDir.Back])
        cls.LEFT_HAND_X_DOWN = cls([cls.ViewDir.Down, cls.ViewDir.Right, cls.ViewDir.Forward])
        cls.LEFT_HAND_Y_UP = cls([cls.ViewDir.Right, cls.ViewDir.Up, cls.ViewDir.Forward])
        cls.LEFT_HAND_Y_DOWN = cls([cls.ViewDir.Right, cls.ViewDir.Down, cls.ViewDir.Back])
        cls.LEFT_HAND_Z_UP = cls([cls.ViewDir.Right, cls.ViewDir.Back, cls.ViewDir.Up])
        cls.LEFT_HAND_Z_DOWN = cls([cls.ViewDir.Right, cls.ViewDir.Forward, cls.ViewDir.Down])
        # <END_GENERATED:definitions>
