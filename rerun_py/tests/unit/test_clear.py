from __future__ import annotations

import rerun.experimental as rr2
from rerun.experimental import cmp as rr_cmp


def test_clear() -> None:
    recursive = True

    print(f"rr2.Clear(\n" f"recursive={recursive}\n" f")")
    arch = rr2.Clear(recursive=recursive)
    print(f"{arch}\n")

    assert arch.recursive == rr_cmp.ClearIsRecursiveBatch([True])


if __name__ == "__main__":
    test_clear()
