from __future__ import annotations

__all__ = [
    "AffixFuzzer1ArrayExt",
    "AffixFuzzer2ArrayExt",
    "AffixFuzzer3ArrayExt",
    "AffixFuzzer4ArrayExt",
    "AffixFuzzer5ArrayExt",
    "AffixFuzzer6ArrayExt",
    "AffixFuzzer7ArrayExt",
]

from typing import Any

import pyarrow as pa


class AffixFuzzer1ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer2ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer3ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer4ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer5ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer6ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()


class AffixFuzzer7ArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        raise NotImplementedError()
