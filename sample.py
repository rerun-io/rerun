from __future__ import annotations

import time
from typing import Any

import numpy as np
import rerun as rr
import torch

LOOPS = 100


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


def main(version: str) -> None:
    shape = (20000,)

    def raw_any(tensor: torch.Tensor):
        return rr.AnyValues(embedding=tensor)

    def numpy_any(tensor: torch.Tensor):
        return rr.AnyValues(embedding=tensor.numpy())

    if version == "raw":
        timing = simple_loop(shape, raw_any, torch.randn)
    elif version == "to_numpy":
        timing = simple_loop(shape, numpy_any, torch.randn)
    elif version == "raw_numpy":
        timing = simple_loop(shape, raw_any, np.random.randn)
    else:
        raise ValueError(f"Unknown version: {version}")

    print(f"{version}:\n\t{timing:.4f}s")


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("version", choices=["raw", "to_numpy", "raw_numpy"], help="Version of AnyValues to test")
    args = parser.parse_args()
    main(args.version)
