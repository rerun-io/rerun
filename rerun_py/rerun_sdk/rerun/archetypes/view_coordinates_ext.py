from __future__ import annotations

from ..components import ViewCoordinates as Component


class ViewCoordinatesExt:
    # <BEGIN_GENERATED>
    # This section is generated by running `/home/jleibs/rerun/./scripts/generate_view_coordinates.py --python`
    ULF = Component([Component.ViewDir.Up, Component.ViewDir.Left, Component.ViewDir.Forward])
    UFL = Component([Component.ViewDir.Up, Component.ViewDir.Forward, Component.ViewDir.Left])
    LUF = Component([Component.ViewDir.Left, Component.ViewDir.Up, Component.ViewDir.Forward])
    LFU = Component([Component.ViewDir.Left, Component.ViewDir.Forward, Component.ViewDir.Up])
    FUL = Component([Component.ViewDir.Forward, Component.ViewDir.Up, Component.ViewDir.Left])
    FLU = Component([Component.ViewDir.Forward, Component.ViewDir.Left, Component.ViewDir.Up])
    ULB = Component([Component.ViewDir.Up, Component.ViewDir.Left, Component.ViewDir.Back])
    UBL = Component([Component.ViewDir.Up, Component.ViewDir.Back, Component.ViewDir.Left])
    LUB = Component([Component.ViewDir.Left, Component.ViewDir.Up, Component.ViewDir.Back])
    LBU = Component([Component.ViewDir.Left, Component.ViewDir.Back, Component.ViewDir.Up])
    BUL = Component([Component.ViewDir.Back, Component.ViewDir.Up, Component.ViewDir.Left])
    BLU = Component([Component.ViewDir.Back, Component.ViewDir.Left, Component.ViewDir.Up])
    URF = Component([Component.ViewDir.Up, Component.ViewDir.Right, Component.ViewDir.Forward])
    UFR = Component([Component.ViewDir.Up, Component.ViewDir.Forward, Component.ViewDir.Right])
    RUF = Component([Component.ViewDir.Right, Component.ViewDir.Up, Component.ViewDir.Forward])
    RFU = Component([Component.ViewDir.Right, Component.ViewDir.Forward, Component.ViewDir.Up])
    FUR = Component([Component.ViewDir.Forward, Component.ViewDir.Up, Component.ViewDir.Right])
    FRU = Component([Component.ViewDir.Forward, Component.ViewDir.Right, Component.ViewDir.Up])
    URB = Component([Component.ViewDir.Up, Component.ViewDir.Right, Component.ViewDir.Back])
    UBR = Component([Component.ViewDir.Up, Component.ViewDir.Back, Component.ViewDir.Right])
    RUB = Component([Component.ViewDir.Right, Component.ViewDir.Up, Component.ViewDir.Back])
    RBU = Component([Component.ViewDir.Right, Component.ViewDir.Back, Component.ViewDir.Up])
    BUR = Component([Component.ViewDir.Back, Component.ViewDir.Up, Component.ViewDir.Right])
    BRU = Component([Component.ViewDir.Back, Component.ViewDir.Right, Component.ViewDir.Up])
    DLF = Component([Component.ViewDir.Down, Component.ViewDir.Left, Component.ViewDir.Forward])
    DFL = Component([Component.ViewDir.Down, Component.ViewDir.Forward, Component.ViewDir.Left])
    LDF = Component([Component.ViewDir.Left, Component.ViewDir.Down, Component.ViewDir.Forward])
    LFD = Component([Component.ViewDir.Left, Component.ViewDir.Forward, Component.ViewDir.Down])
    FDL = Component([Component.ViewDir.Forward, Component.ViewDir.Down, Component.ViewDir.Left])
    FLD = Component([Component.ViewDir.Forward, Component.ViewDir.Left, Component.ViewDir.Down])
    DLB = Component([Component.ViewDir.Down, Component.ViewDir.Left, Component.ViewDir.Back])
    DBL = Component([Component.ViewDir.Down, Component.ViewDir.Back, Component.ViewDir.Left])
    LDB = Component([Component.ViewDir.Left, Component.ViewDir.Down, Component.ViewDir.Back])
    LBD = Component([Component.ViewDir.Left, Component.ViewDir.Back, Component.ViewDir.Down])
    BDL = Component([Component.ViewDir.Back, Component.ViewDir.Down, Component.ViewDir.Left])
    BLD = Component([Component.ViewDir.Back, Component.ViewDir.Left, Component.ViewDir.Down])
    DRF = Component([Component.ViewDir.Down, Component.ViewDir.Right, Component.ViewDir.Forward])
    DFR = Component([Component.ViewDir.Down, Component.ViewDir.Forward, Component.ViewDir.Right])
    RDF = Component([Component.ViewDir.Right, Component.ViewDir.Down, Component.ViewDir.Forward])
    RFD = Component([Component.ViewDir.Right, Component.ViewDir.Forward, Component.ViewDir.Down])
    FDR = Component([Component.ViewDir.Forward, Component.ViewDir.Down, Component.ViewDir.Right])
    FRD = Component([Component.ViewDir.Forward, Component.ViewDir.Right, Component.ViewDir.Down])
    DRB = Component([Component.ViewDir.Down, Component.ViewDir.Right, Component.ViewDir.Back])
    DBR = Component([Component.ViewDir.Down, Component.ViewDir.Back, Component.ViewDir.Right])
    RDB = Component([Component.ViewDir.Right, Component.ViewDir.Down, Component.ViewDir.Back])
    RBD = Component([Component.ViewDir.Right, Component.ViewDir.Back, Component.ViewDir.Down])
    BDR = Component([Component.ViewDir.Back, Component.ViewDir.Down, Component.ViewDir.Right])
    BRD = Component([Component.ViewDir.Back, Component.ViewDir.Right, Component.ViewDir.Down])
    RIGHT_HAND_POS_X_UP = Component([Component.ViewDir.Up, Component.ViewDir.Right, Component.ViewDir.Forward])
    RIGHT_HAND_NEG_X_UP = Component([Component.ViewDir.Down, Component.ViewDir.Right, Component.ViewDir.Back])
    RIGHT_HAND_POS_Y_UP = Component([Component.ViewDir.Right, Component.ViewDir.Up, Component.ViewDir.Back])
    RIGHT_HAND_NEG_Y_UP = Component([Component.ViewDir.Right, Component.ViewDir.Down, Component.ViewDir.Forward])
    RIGHT_HAND_POS_Z_UP = Component([Component.ViewDir.Right, Component.ViewDir.Forward, Component.ViewDir.Up])
    RIGHT_HAND_NEG_Z_UP = Component([Component.ViewDir.Right, Component.ViewDir.Back, Component.ViewDir.Down])
    LEFT_HAND_POS_X_UP = Component([Component.ViewDir.Up, Component.ViewDir.Right, Component.ViewDir.Back])
    LEFT_HAND_NEG_X_UP = Component([Component.ViewDir.Down, Component.ViewDir.Right, Component.ViewDir.Forward])
    LEFT_HAND_POS_Y_UP = Component([Component.ViewDir.Right, Component.ViewDir.Up, Component.ViewDir.Forward])
    LEFT_HAND_NEG_Y_UP = Component([Component.ViewDir.Right, Component.ViewDir.Down, Component.ViewDir.Back])
    LEFT_HAND_POS_Z_UP = Component([Component.ViewDir.Right, Component.ViewDir.Back, Component.ViewDir.Up])
    LEFT_HAND_NEG_Z_UP = Component([Component.ViewDir.Right, Component.ViewDir.Forward, Component.ViewDir.Down])
    # <END_GENERATED>
