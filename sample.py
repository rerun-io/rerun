from __future__ import annotations

import time
from typing import Any

import numpy as np
import rerun as rr
import torch

LOOPS = 10


def simple_loop(shape: Any, callback: Any, data_generator: Any) -> float:
    data = data_generator(*shape)
    total_time = 0.0
    all_data = []
    count = 0
    for _ in range(LOOPS):
        count += 1
        start = time.time()
        all_data.append(callback(data))
        total_time += time.time() - start
    print(f"Loop {count}")
    print(len(all_data[0].component_batches))
    return total_time


def main() -> None:
    shape = (2000,)

    def raw_any(tensor: torch.Tensor):
        return rr.AnyValues(embedding=tensor)

    def numpy_any(tensor: torch.Tensor):
        return rr.AnyValues(embedding=tensor.numpy())

    print("Numpy")
    numpy = simple_loop(shape, numpy_any, torch.randn)
    print("Orig")
    orig = simple_loop(shape, raw_any, torch.randn)
    print("Actual")
    actual_numpy = simple_loop(shape, raw_any, np.random.randn)

    print(
        f"Original: {orig:.4f}s, Numpy: {numpy:.4f}s, Actual Numpy: {actual_numpy:.4f}s, Speedup: {orig / numpy:.2f}x"
    )


if __name__ == "__main__":
    main()
