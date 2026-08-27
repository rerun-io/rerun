"""YUV420 collation and device-side RGB conversion for video decoder outputs."""

from __future__ import annotations

import math
import warnings
from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal, cast

import torch
import torch.nn.functional as F
from torch.utils.data import default_collate, get_worker_info

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence

ColorSpace = Literal["bt601", "bt709", "bt2020", "unspecified"]
ColorRange = Literal["limited", "full", "unspecified"]


@dataclass(frozen=True, slots=True)
class Yuv420Frame:
    """
    One frame, frame window, or collated batch in planar YUV420 form.

    `y` has shape `[..., 1, H, W]` and `uv` has shape
    `[..., 2, ceil(H / 2), ceil(W / 2)]`. Both tensors are `uint8`.
    `uv[..., 0, :, :]` is U and `uv[..., 1, :, :]` is V. Leading
    dimensions may contain time and, after collation, batch.

    Use [`Yuv420Frame.stack`][rerun.experimental.dataloader.Yuv420Frame.stack] inside a custom collate function, or the
    convenience [`Yuv420Collator`][rerun.experimental.dataloader.Yuv420Collator].
    The standard PyTorch collator does not know how to stack the planes and
    metadata.
    """

    y: torch.Tensor
    uv: torch.Tensor
    color_space: ColorSpace
    color_range: ColorRange

    def __post_init__(self) -> None:
        if self.y.dtype is not torch.uint8 or self.uv.dtype is not torch.uint8:
            raise TypeError("YUV420 planes must have dtype torch.uint8")
        if self.y.device != self.uv.device:
            raise ValueError("YUV420 planes must be on the same device")
        if self.y.ndim < 3 or self.uv.ndim != self.y.ndim:
            raise ValueError("YUV420 planes must have matching […, C, H, W] ranks")
        if self.y.shape[:-3] != self.uv.shape[:-3] or self.y.shape[-3] != 1 or self.uv.shape[-3] != 2:
            raise ValueError("YUV420 planes must have matching leading dimensions and 1 Y / 2 UV channels")
        height, width = self.y.shape[-2:]
        if self.uv.shape[-2:] != (math.ceil(height / 2), math.ceil(width / 2)):
            raise ValueError("YUV420 chroma dimensions must be ceil(H / 2) by ceil(W / 2)")
        if self.color_space not in ("bt601", "bt709", "bt2020", "unspecified"):
            raise ValueError(f"Unsupported YUV color space: {self.color_space!r}")
        if self.color_range not in ("limited", "full", "unspecified"):
            raise ValueError(f"Unsupported YUV color range: {self.color_range!r}")

    @classmethod
    def stack(cls, frames: Sequence[Yuv420Frame]) -> Yuv420Frame:
        """
        Stack frames or windows along a new leading batch dimension.

        This is the composable building block for application-specific collate
        functions. All inputs must have identical shapes and color metadata.
        Stacking materializes window views, so the returned batch no longer
        shares decoder frame-bank storage with its inputs.
        """
        if not frames:
            raise ValueError("Cannot stack an empty sequence of YUV420 frames")
        color_space = frames[0].color_space
        color_range = frames[0].color_range
        if any((frame.color_space, frame.color_range) != (color_space, color_range) for frame in frames[1:]):
            metadata = sorted({(frame.color_space, frame.color_range) for frame in frames})
            raise ValueError(f"A YUV420 batch must have uniform color metadata, got {metadata!r}")
        return cls(
            y=torch.stack([frame.y for frame in frames]),
            uv=torch.stack([frame.uv for frame in frames]),
            color_space=color_space,
            color_range=color_range,
        )

    def clone(self) -> Yuv420Frame:
        """Return an independent copy of both planes."""
        return Yuv420Frame(
            y=self.y.clone(),
            uv=self.uv.clone(),
            color_space=self.color_space,
            color_range=self.color_range,
        )

    def pin_memory(self) -> Yuv420Frame:
        """Pin both planes so a DataLoader can transfer them asynchronously."""
        return Yuv420Frame(
            y=self.y.pin_memory(),
            uv=self.uv.pin_memory(),
            color_space=self.color_space,
            color_range=self.color_range,
        )

    def to_rgb(
        self,
        device: torch.device | str | None = None,
        *,
        dtype: torch.dtype = torch.float32,
        normalize: bool = True,
        non_blocking: bool = False,
        color_space: Literal["bt601", "bt709", "bt2020"] | None = None,
        color_range: Literal["limited", "full"] | None = None,
    ) -> torch.Tensor:
        """
        Transfer and convert to a contiguous `[..., 3, H, W]` RGB tensor.

        Conversion goes directly from CPU `uint8` YUV into the requested
        floating-point dtype on `device`, so an intermediate RGB `uint8`
        allocation is not required. If `normalize` is true, output values are
        in `[0, 1]`; otherwise they are in `[0, 255]`.

        Chroma is expanded with nearest-neighbor sampling to stay close to
        FFmpeg's default `rgb24` conversion. Conversion can still differ by a
        few intensity levels because FFmpeg uses integer arithmetic.

        `color_space` and `color_range` override the metadata carried by the
        decoded frame. Unspecified metadata falls back to BT.601 limited range
        with a warning, matching FFmpeg's conventional SD-video default.
        """
        if not dtype.is_floating_point:
            raise TypeError(f"YUV to RGB conversion requires a floating-point dtype, got {dtype}")

        target_device = torch.device(device) if device is not None else self.y.device
        if target_device.type == "cuda" and get_worker_info() is not None:
            raise RuntimeError(
                "CUDA YUV conversion cannot run inside a DataLoader worker; "
                "collate to pinned CPU YUV and call Yuv420Frame.to_rgb in the training process"
            )

        resolved_space, resolved_range = _resolve_color_metadata(
            self,
            color_space=color_space,
            color_range=color_range,
        )
        y = self.y.to(device=target_device, dtype=dtype, non_blocking=non_blocking)
        uv = self.uv.to(device=target_device, dtype=dtype, non_blocking=non_blocking)

        height, width = y.shape[-2:]
        leading_shape = y.shape[:-3]
        uv = F.interpolate(
            uv.reshape(-1, 2, *uv.shape[-2:]),
            size=(height, width),
            mode="nearest",
        ).reshape(*leading_shape, 2, height, width)

        if resolved_range == "limited":
            luminance = (y - 16.0) * (255.0 / 219.0)
            chroma = (uv - 128.0) * (255.0 / 224.0)
        else:
            luminance = y
            chroma = uv - 128.0

        kr, kb = {
            "bt601": (0.2990, 0.1140),
            "bt709": (0.2126, 0.0722),
            "bt2020": (0.2627, 0.0593),
        }[resolved_space]
        kg = 1.0 - kr - kb
        cb = chroma[..., 0:1, :, :]
        cr = chroma[..., 1:2, :, :]
        red = luminance + (2.0 - 2.0 * kr) * cr
        green = luminance - kb * (2.0 - 2.0 * kb) / kg * cb - kr * (2.0 - 2.0 * kr) / kg * cr
        blue = luminance + (2.0 - 2.0 * kb) * cb
        rgb = torch.cat((red, green, blue), dim=-3).clamp_(0.0, 255.0)
        if normalize:
            rgb.mul_(1.0 / 255.0)
        return rgb


class Yuv420Collator:
    """
    Collate dataset mappings containing [`Yuv420Frame`][rerun.experimental.dataloader.Yuv420Frame] values.

    Without `device`, YUV planes remain compact CPU `uint8` tensors. This is
    the intended configuration with `DataLoader(num_workers>0,
    pin_memory=True)`: convert each collated `Yuv420Frame` in the training
    process using [`Yuv420Frame.to_rgb`][rerun.experimental.dataloader.Yuv420Frame.to_rgb].

    Examples
    --------
    Custom collation can handle each non-video field however the application
    requires:

    ```python
    def collate(samples):
        return {
            "video": Yuv420Frame.stack([sample["video"] for sample in samples]),
            "state": custom_state_collation(samples),
        }
    ```

    The convenience collator applies PyTorch's default collation to non-video
    fields:

    ```python
    loader = DataLoader(dataset, collate_fn=Yuv420Collator(), pin_memory=True)
    for batch in loader:
        batch["video"] = batch["video"].to_rgb("cuda", dtype=torch.float16, non_blocking=True)
    ```

    """

    def __call__(self, samples: Sequence[Mapping[str, object]]) -> dict[str, object]:
        """Stack a batch of sample mappings, preserving YUV for device-side conversion."""
        if not samples:
            return {}
        keys = samples[0].keys()
        if any(sample.keys() != keys for sample in samples[1:]):
            raise ValueError("All samples must contain the same fields")
        return {key: self._collate_values([sample[key] for sample in samples]) for key in keys}

    @staticmethod
    def _collate_values(values: list[object]) -> object:
        if any(value is None for value in values):
            raise ValueError("Cannot collate missing field values; filter or replace None samples before collation")
        first = values[0]
        if isinstance(first, Yuv420Frame):
            if not all(isinstance(value, Yuv420Frame) for value in values):
                raise TypeError("A field cannot mix Yuv420Frame values with other types")
            return Yuv420Frame.stack(cast("list[Yuv420Frame]", values))
        return cast("object", default_collate(values))


def _resolve_color_metadata(
    frame: Yuv420Frame,
    *,
    color_space: Literal["bt601", "bt709", "bt2020"] | None,
    color_range: Literal["limited", "full"] | None,
) -> tuple[Literal["bt601", "bt709", "bt2020"], Literal["limited", "full"]]:
    resolved_space = color_space if color_space is not None else frame.color_space
    resolved_range = color_range if color_range is not None else frame.color_range
    defaulted: list[str] = []
    if resolved_space == "unspecified":
        resolved_space = "bt601"
        defaulted.append("color space to BT.601")
    if resolved_range == "unspecified":
        resolved_range = "limited"
        defaulted.append("color range to limited")
    if defaulted:
        warnings.warn(
            f"YUV frame metadata is unspecified; defaulting {' and '.join(defaulted)}. "
            "Pass explicit color_space/color_range overrides if the source uses different values.",
            stacklevel=3,
        )
    return resolved_space, resolved_range
