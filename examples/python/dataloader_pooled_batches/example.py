"""Exercise pooled batch buffers without requiring a server or dataset download."""

from __future__ import annotations

import argparse

import torch
from torch.utils.data import DataLoader, Dataset

from rerun.experimental.dataloader import PooledBatchCollator


class Samples(Dataset[dict[str, torch.Tensor]]):
    def __len__(self) -> int:
        return 128

    def __getitem__(self, index: int) -> dict[str, torch.Tensor]:
        return {
            "camera": torch.full((6, 3, 64, 96), index % 256, dtype=torch.uint8),
            "state": torch.arange(6, dtype=torch.float32) + index,
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workers", type=int, default=2)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cuda" if torch.cuda.is_available() else "cpu")
    args = parser.parse_args()
    if args.workers < 0:
        parser.error("--workers must be non-negative")
    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("--device cuda requires a CUDA accelerator")
    device = torch.device(args.device)

    loader = DataLoader(
        Samples(),
        batch_size=8,
        num_workers=args.workers,
        multiprocessing_context="spawn" if args.workers else None,
        pin_memory=True,
        collate_fn=PooledBatchCollator(packed=True, slots_per_field=4, max_pool_bytes=64 * 1024**2),
    )
    retained = None
    count = 0
    for batch in loader:
        if retained is None:
            retained = batch["camera"][0]
        device_batch = {name: tensor.to(device, non_blocking=True) for name, tensor in batch.items()}
        if device.type == "cuda":
            # Shared CPU storage must remain alive until non-blocking copies finish.
            torch.cuda.synchronize(device)
        count += len(device_batch["camera"])
    assert retained is not None
    torch.testing.assert_close(retained, torch.zeros_like(retained), rtol=0, atol=0)
    print(f"Loaded {count} samples; the retained first camera tensor is unchanged.")


if __name__ == "__main__":
    main()
