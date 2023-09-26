from __future__ import annotations

import rerun as rr
from rerun.components import ClearIsRecursiveBatch


def test_clear() -> None:
    recursive = True

    print(f"rr.Clear(\n" f"recursive={recursive}\n" f")")
    arch = rr.Clear(recursive=recursive)
    print(f"{arch}\n")

    assert arch.recursive == ClearIsRecursiveBatch([True])


if __name__ == "__main__":
    test_clear()
