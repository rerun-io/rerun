# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from dataclasses import dataclass, field

from ._base import Archetype

__all__ = ["AffixFuzzer1"]

from .. import components

## --- AffixFuzzer1 --- ##


@dataclass
class AffixFuzzer1(Archetype):
    fuzz1001: components.AffixFuzzer1Array = field(metadata={"component": "primary"})
    fuzz1002: components.AffixFuzzer2Array = field(metadata={"component": "primary"})
    fuzz1003: components.AffixFuzzer3Array = field(metadata={"component": "primary"})
    fuzz1004: components.AffixFuzzer4Array = field(metadata={"component": "primary"})
    fuzz1005: components.AffixFuzzer5Array = field(metadata={"component": "primary"})
    fuzz1006: components.AffixFuzzer6Array = field(metadata={"component": "primary"})
    fuzz1007: components.AffixFuzzer7Array = field(metadata={"component": "primary"})
    fuzz1101: components.AffixFuzzer1Array = field(metadata={"component": "primary"})
    fuzz1102: components.AffixFuzzer2Array = field(metadata={"component": "primary"})
    fuzz1103: components.AffixFuzzer3Array = field(metadata={"component": "primary"})
    fuzz1104: components.AffixFuzzer4Array = field(metadata={"component": "primary"})
    fuzz1105: components.AffixFuzzer5Array = field(metadata={"component": "primary"})
    fuzz1106: components.AffixFuzzer6Array = field(metadata={"component": "primary"})
    fuzz1107: components.AffixFuzzer7Array = field(metadata={"component": "primary"})
    fuzz2001: components.AffixFuzzer1Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2002: components.AffixFuzzer2Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2003: components.AffixFuzzer3Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2004: components.AffixFuzzer4Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2005: components.AffixFuzzer5Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2006: components.AffixFuzzer6Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2007: components.AffixFuzzer7Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2101: components.AffixFuzzer1Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2102: components.AffixFuzzer2Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2103: components.AffixFuzzer3Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2104: components.AffixFuzzer4Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2105: components.AffixFuzzer5Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2106: components.AffixFuzzer6Array | None = field(default=None, metadata={"component": "secondary"})
    fuzz2107: components.AffixFuzzer7Array | None = field(default=None, metadata={"component": "secondary"})

    def __str__(self) -> str:
        s = f"rr.{type(self).__name__}(\n"

        from dataclasses import fields

        for fld in fields(self):
            if "component" in fld.metadata:
                comp: components.Component = getattr(self, fld.name)
                if datatype := getattr(comp, "type"):
                    name = comp.extension_name
                    typ = datatype.storage_type
                    s += f"  {name}<{typ}>(\n    {comp.to_pylist()}\n  )\n"

        s += ")"

        return s

    def __repr__(self) -> str:
        return str(self)

    def __init__(
        self,
        fuzz1001: components.AffixFuzzer1Like,
        fuzz1002: components.AffixFuzzer2Like,
        fuzz1003: components.AffixFuzzer3Like,
        fuzz1004: components.AffixFuzzer4Like,
        fuzz1005: components.AffixFuzzer5Like,
        fuzz1006: components.AffixFuzzer6Like,
        fuzz1007: components.AffixFuzzer7Like,
        fuzz1101: components.AffixFuzzer1ArrayLike,
        fuzz1102: components.AffixFuzzer2ArrayLike,
        fuzz1103: components.AffixFuzzer3ArrayLike,
        fuzz1104: components.AffixFuzzer4ArrayLike,
        fuzz1105: components.AffixFuzzer5ArrayLike,
        fuzz1106: components.AffixFuzzer6ArrayLike,
        fuzz1107: components.AffixFuzzer7ArrayLike,
        *,
        fuzz2001: components.AffixFuzzer1Like | None = None,
        fuzz2002: components.AffixFuzzer2Like | None = None,
        fuzz2003: components.AffixFuzzer3Like | None = None,
        fuzz2004: components.AffixFuzzer4Like | None = None,
        fuzz2005: components.AffixFuzzer5Like | None = None,
        fuzz2006: components.AffixFuzzer6Like | None = None,
        fuzz2007: components.AffixFuzzer7Like | None = None,
        fuzz2101: components.AffixFuzzer1ArrayLike | None = None,
        fuzz2102: components.AffixFuzzer2ArrayLike | None = None,
        fuzz2103: components.AffixFuzzer3ArrayLike | None = None,
        fuzz2104: components.AffixFuzzer4ArrayLike | None = None,
        fuzz2105: components.AffixFuzzer5ArrayLike | None = None,
        fuzz2106: components.AffixFuzzer6ArrayLike | None = None,
        fuzz2107: components.AffixFuzzer7ArrayLike | None = None,
    ) -> None:
        # Required components
        self.fuzz1001 = components.AffixFuzzer1Array.from_similar(fuzz1001)
        self.fuzz1002 = components.AffixFuzzer2Array.from_similar(fuzz1002)
        self.fuzz1003 = components.AffixFuzzer3Array.from_similar(fuzz1003)
        self.fuzz1004 = components.AffixFuzzer4Array.from_similar(fuzz1004)
        self.fuzz1005 = components.AffixFuzzer5Array.from_similar(fuzz1005)
        self.fuzz1006 = components.AffixFuzzer6Array.from_similar(fuzz1006)
        self.fuzz1007 = components.AffixFuzzer7Array.from_similar(fuzz1007)
        self.fuzz1101 = components.AffixFuzzer1Array.from_similar(fuzz1101)
        self.fuzz1102 = components.AffixFuzzer2Array.from_similar(fuzz1102)
        self.fuzz1103 = components.AffixFuzzer3Array.from_similar(fuzz1103)
        self.fuzz1104 = components.AffixFuzzer4Array.from_similar(fuzz1104)
        self.fuzz1105 = components.AffixFuzzer5Array.from_similar(fuzz1105)
        self.fuzz1106 = components.AffixFuzzer6Array.from_similar(fuzz1106)
        self.fuzz1107 = components.AffixFuzzer7Array.from_similar(fuzz1107)

        # Optional components

        self.fuzz2001 = components.AffixFuzzer1Array.from_similar(fuzz2001)
        self.fuzz2002 = components.AffixFuzzer2Array.from_similar(fuzz2002)
        self.fuzz2003 = components.AffixFuzzer3Array.from_similar(fuzz2003)
        self.fuzz2004 = components.AffixFuzzer4Array.from_similar(fuzz2004)
        self.fuzz2005 = components.AffixFuzzer5Array.from_similar(fuzz2005)
        self.fuzz2006 = components.AffixFuzzer6Array.from_similar(fuzz2006)
        self.fuzz2007 = components.AffixFuzzer7Array.from_similar(fuzz2007)
        self.fuzz2101 = components.AffixFuzzer1Array.from_similar(fuzz2101)
        self.fuzz2102 = components.AffixFuzzer2Array.from_similar(fuzz2102)
        self.fuzz2103 = components.AffixFuzzer3Array.from_similar(fuzz2103)
        self.fuzz2104 = components.AffixFuzzer4Array.from_similar(fuzz2104)
        self.fuzz2105 = components.AffixFuzzer5Array.from_similar(fuzz2105)
        self.fuzz2106 = components.AffixFuzzer6Array.from_similar(fuzz2106)
        self.fuzz2107 = components.AffixFuzzer7Array.from_similar(fuzz2107)
