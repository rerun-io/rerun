from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..components import ViewCoordinates as Component

if TYPE_CHECKING:
    from . import ViewCoordinates


class ViewCoordinatesExt:
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
        cls.ULF = Component.ULF
        cls.UFL = Component.UFL
        cls.LUF = Component.LUF
        cls.LFU = Component.LFU
        cls.FUL = Component.FUL
        cls.FLU = Component.FLU
        cls.ULB = Component.ULB
        cls.UBL = Component.UBL
        cls.LUB = Component.LUB
        cls.LBU = Component.LBU
        cls.BUL = Component.BUL
        cls.BLU = Component.BLU
        cls.URF = Component.URF
        cls.UFR = Component.UFR
        cls.RUF = Component.RUF
        cls.RFU = Component.RFU
        cls.FUR = Component.FUR
        cls.FRU = Component.FRU
        cls.URB = Component.URB
        cls.UBR = Component.UBR
        cls.RUB = Component.RUB
        cls.RBU = Component.RBU
        cls.BUR = Component.BUR
        cls.BRU = Component.BRU
        cls.DLF = Component.DLF
        cls.DFL = Component.DFL
        cls.LDF = Component.LDF
        cls.LFD = Component.LFD
        cls.FDL = Component.FDL
        cls.FLD = Component.FLD
        cls.DLB = Component.DLB
        cls.DBL = Component.DBL
        cls.LDB = Component.LDB
        cls.LBD = Component.LBD
        cls.BDL = Component.BDL
        cls.BLD = Component.BLD
        cls.DRF = Component.DRF
        cls.DFR = Component.DFR
        cls.RDF = Component.RDF
        cls.RFD = Component.RFD
        cls.FDR = Component.FDR
        cls.FRD = Component.FRD
        cls.DRB = Component.DRB
        cls.DBR = Component.DBR
        cls.RDB = Component.RDB
        cls.RBD = Component.RBD
        cls.BDR = Component.BDR
        cls.BRD = Component.BRD
        cls.RIGHT_HAND_X_UP = Component.RIGHT_HAND_X_UP
        cls.RIGHT_HAND_X_DOWN = Component.RIGHT_HAND_X_DOWN
        cls.RIGHT_HAND_Y_UP = Component.RIGHT_HAND_Y_UP
        cls.RIGHT_HAND_Y_DOWN = Component.RIGHT_HAND_Y_DOWN
        cls.RIGHT_HAND_Z_UP = Component.RIGHT_HAND_Z_UP
        cls.RIGHT_HAND_Z_DOWN = Component.RIGHT_HAND_Z_DOWN
        cls.LEFT_HAND_X_UP = Component.LEFT_HAND_X_UP
        cls.LEFT_HAND_X_DOWN = Component.LEFT_HAND_X_DOWN
        cls.LEFT_HAND_Y_UP = Component.LEFT_HAND_Y_UP
        cls.LEFT_HAND_Y_DOWN = Component.LEFT_HAND_Y_DOWN
        cls.LEFT_HAND_Z_UP = Component.LEFT_HAND_Z_UP
        cls.LEFT_HAND_Z_DOWN = Component.LEFT_HAND_Z_DOWN
        # <END_GENERATED:definitions>
