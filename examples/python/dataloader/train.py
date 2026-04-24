"""Train a LeRobot ACT policy using the Rerun dataloader.

Demonstrates how to stream robot trajectory data from Rerun's catalog
into an imitation learning policy (Action Chunking Transformers).

The Rerun dataloader's Column.window feature fetches future action chunks in a single query per batch.
"""

from __future__ import annotations

import argparse
import time
from pathlib import Path

import torch
import torch.nn.functional as F
from lerobot.configs.types import FeatureType, NormalizationMode, PolicyFeature
from lerobot.policies.act.configuration_act import ACTConfig
from lerobot.policies.act.modeling_act import ACTPolicy
from torch.utils.data import DataLoader

from rerun._tracing import tracing_scope, with_tracing
from rerun.catalog import CatalogClient
from rerun.experimental.dataloader import (
    Column,
    DataSource,
    NumericDecoder,
    RerunIterableDataset,
    RerunMapDataset,
    VideoFrameDecoder,
)

CHECKPOINT_DIR = Path(__file__).resolve().parent / "act_checkpoint"

IMAGE_H = 32
IMAGE_W = 128
CAMERAS = ("laptop", "phone", "side")
IMAGE_KEYS = tuple(f"observation.images.{cam}" for cam in CAMERAS)

CHUNK_SIZE = 50
EPOCHS = 5
BATCH_SIZE = 8
LR = 1e-5
NUM_WORKERS = 8


class CollateFn:
    """Picklable collate callable for PyTorch DataLoader multiprocessing."""

    def __init__(self, chunk_size: int, state_dim: int) -> None:
        self.chunk_size = chunk_size
        self.state_dim = state_dim

    @with_tracing("CollateFn")
    def __call__(self, samples: list[dict[str, torch.Tensor]]) -> dict[str, torch.Tensor]:
        batch_size = len(samples)

        states = torch.stack([s["state"] for s in samples]).float()

        # Future action chunks: reshape windowed flat tensors
        actions = torch.stack([s["action"].reshape(self.chunk_size, self.state_dim) for s in samples]).float()

        batch: dict[str, torch.Tensor] = {
            "observation.state": states,
            "action": actions,
            "action_is_pad": torch.zeros(batch_size, self.chunk_size, dtype=torch.bool),
        }
        # Per-camera images: (3, H, W) uint8 -> float in [0, 1], resized to (IMAGE_H, IMAGE_W)
        for cam, key in zip(CAMERAS, IMAGE_KEYS):
            imgs = torch.stack([s[f"image_{cam}"] for s in samples]).float() / 255.0
            batch[key] = F.interpolate(imgs, size=(IMAGE_H, IMAGE_W), mode="bilinear", align_corners=False)
        return batch


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--catalog-url",
        default="rerun+http://127.0.0.1:51234",
        help="Rerun catalog URL",
    )
    parser.add_argument(
        "--dataset",
        default="rerun_so101-pick-and-place",
        help="Dataset name in the catalog",
    )
    parser.add_argument("--num-segments", type=int, default=3, help="Number of segments to use (0 for all)")
    parser.add_argument("--epochs", type=int, default=EPOCHS, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=BATCH_SIZE, help="Training batch size")
    parser.add_argument("--num-workers", type=int, default=NUM_WORKERS, help="DataLoader worker processes")
    parser.add_argument("--lr", type=float, default=LR, help="Learning rate")
    parser.add_argument(
        "--dataset-style",
        choices=("iterable", "map"),
        default="iterable",
        help="Which Rerun dataset class to use: 'iterable' (RerunIterableDataset, internal shuffling) "
        "or 'map' (RerunMapDataset, random access via DataLoader samplers).",
    )
    parser.add_argument(
        "--checkpoint-dir",
        type=Path,
        default=CHECKPOINT_DIR,
        help="Directory to save the trained policy checkpoint",
    )
    return parser.parse_args()


@with_tracing("main")
def main() -> None:
    args = parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    client = CatalogClient(args.catalog_url)
    dataset_entry = client.get_dataset(args.dataset)

    all_segments = dataset_entry.segment_ids()
    segments = all_segments if args.num_segments == 0 else all_segments[: args.num_segments]
    print(f"Using {len(segments)} segments")

    source = DataSource(dataset_entry, segments=segments)

    columns = {
        "state": Column("/observation.state:Scalars:scalars", decode=NumericDecoder()),
        "action": Column(
            "/action:Scalars:scalars",
            decode=NumericDecoder(),
            window=(1, CHUNK_SIZE),
        ),
        "image_laptop": Column(
            "/observation.images.laptop:VideoStream:sample",
            decode=VideoFrameDecoder(codec="av1", keyframe_interval=2),
        ),
        "image_phone": Column(
            "/observation.images.phone:VideoStream:sample",
            decode=VideoFrameDecoder(codec="av1", keyframe_interval=2),
        ),
        "image_side": Column(
            "/observation.images.side:VideoStream:sample",
            decode=VideoFrameDecoder(codec="av1", keyframe_interval=2),
        ),
    }

    ds: RerunIterableDataset | RerunMapDataset
    if args.dataset_style == "map":
        ds = RerunMapDataset(source=source, index="frame_index", columns=columns)
    else:
        ds = RerunIterableDataset(source=source, index="frame_index", columns=columns, fetch_size=512)
    print(f"Using {args.dataset_style} dataset with {len(ds)} samples (after window trimming)")

    # IterableDataset doesn't support indexing, so probe shape via iteration.
    state_dim = next(iter(ds))["state"].shape[0]
    action_dim = state_dim
    print(f"Dimensions: {state_dim=}, {action_dim=}")

    config = ACTConfig(
        chunk_size=CHUNK_SIZE,
        n_action_steps=CHUNK_SIZE,
        use_vae=True,
        kl_weight=10.0,
        dim_model=256,
        n_heads=8,
        dim_feedforward=1024,
        n_encoder_layers=4,
        n_decoder_layers=1,
        latent_dim=32,
        n_vae_encoder_layers=4,
        dropout=0.1,
        vision_backbone="resnet18",
        pretrained_backbone_weights=None,
        normalization_mapping={
            "STATE": NormalizationMode.MEAN_STD,
            "VISUAL": NormalizationMode.MEAN_STD,
            "ACTION": NormalizationMode.MEAN_STD,
        },
        input_features={
            "observation.state": PolicyFeature(type=FeatureType.STATE, shape=(state_dim,)),
            **{key: PolicyFeature(type=FeatureType.VISUAL, shape=(3, IMAGE_H, IMAGE_W)) for key in IMAGE_KEYS},
        },
        output_features={
            "action": PolicyFeature(type=FeatureType.ACTION, shape=(action_dim,)),
        },
    )

    policy = ACTPolicy(config)
    policy.train()
    policy.to(device)
    print(f"ACT policy created ({sum(p.numel() for p in policy.parameters()):,} parameters, device={device})")

    optimizer = torch.optim.AdamW(
        policy.get_optim_params(),
        lr=args.lr,
        weight_decay=1e-4,
    )

    collate_fn = CollateFn(CHUNK_SIZE, state_dim)
    # For the map-style dataset, shuffling is driven by the DataLoader's default RandomSampler.
    # Swap in `sampler=DistributedSampler(ds)` (and call `sampler.set_epoch(epoch)` each epoch)
    # for multi-node training, or plug in any other PyTorch sampler.
    loader = DataLoader(
        ds,
        batch_size=args.batch_size,
        shuffle=isinstance(ds, RerunMapDataset),
        num_workers=args.num_workers,
        collate_fn=collate_fn,
        persistent_workers=True,
        prefetch_factor=8,
    )

    num_batches = len(loader)
    print(f"\nTraining for {args.epochs} epochs, {num_batches} batches/epoch, batch_size={args.batch_size}\n")

    for epoch in range(args.epochs):
        with tracing_scope(f"epoch {epoch}"):
            if isinstance(ds, RerunIterableDataset):
                ds.set_epoch(epoch)

            total_loss = 0.0
            total_l1 = 0.0
            total_kld = 0.0
            n = 0

            t_last_print = time.perf_counter()
            data_sum = 0.0
            model_sum = 0.0
            t_data_start = time.perf_counter()
            for batch in loader:
                data_time = time.perf_counter() - t_data_start

                t_model_start = time.perf_counter()
                batch = {k: v.to(device) for k, v in batch.items()}
                loss, loss_dict = policy.forward(batch)
                optimizer.zero_grad()
                loss.backward()
                optimizer.step()
                model_time = time.perf_counter() - t_model_start

                total_loss += loss.item()
                total_l1 += loss_dict["l1_loss"]
                total_kld += loss_dict.get("kld_loss", 0.0)
                data_sum += data_time
                model_sum += model_time
                n += 1

                if n % 10 == 0 or n == 1:
                    now = time.perf_counter()
                    since_last = now - t_last_print
                    t_last_print = now
                    print(
                        f"  epoch {epoch + 1}/{args.epochs}  batch {n}/{num_batches}"
                        f"  loss={loss.item():.4f}"
                        f"  data={data_sum:.1f}s model={model_sum:.1f}s"
                        f"  since_last={since_last:.1f}s",
                        flush=True,
                    )
                    data_sum = 0.0
                    model_sum = 0.0

                t_data_start = time.perf_counter()

            avg_loss = total_loss / max(n, 1)
            avg_l1 = total_l1 / max(n, 1)
            avg_kld = total_kld / max(n, 1)
            print(f"Epoch {epoch + 1}/{args.epochs}  loss={avg_loss:.4f}  l1={avg_l1:.4f}  kld={avg_kld:.4f}")

    with tracing_scope("save_pretrained"):
        policy.save_pretrained(str(args.checkpoint_dir))
    print(f"\nSaved checkpoint to {args.checkpoint_dir}")


if __name__ == "__main__":
    main()
