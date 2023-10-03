from __future__ import annotations

import rerun as rr
from rerun.components import ClearIsRecursiveBatch


def test_clear() -> None:
    recursive = True

    print(f"rr.Clear(\n" f"recursive={recursive}\n" f")")
    arch = rr.Clear(recursive=recursive)
    print(f"{arch}\n")

    assert arch.is_recursive == ClearIsRecursiveBatch([True])


def test_clear_factory_methods() -> None:
    assert rr.Clear(recursive=True) == rr.Clear.recursive()
    assert rr.Clear(recursive=False) == rr.Clear.flat()


if __name__ == "__main__":
    test_clear()
