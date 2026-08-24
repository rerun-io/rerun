"""Tests for YUV420 collation and RGB conversion."""

from __future__ import annotations

import pickle

import pytest
import torch
from rerun.experimental.dataloader import Yuv420Collator, Yuv420Frame


def _frame(*, color_space: str = "bt709", color_range: str = "full") -> Yuv420Frame:
    return Yuv420Frame(
        y=torch.full((1, 4, 4), 80, dtype=torch.uint8),
        uv=torch.tensor([[[64, 192], [96, 160]], [[192, 64], [160, 96]]], dtype=torch.uint8),
        color_space=color_space,  # type: ignore[arg-type]
        color_range=color_range,  # type: ignore[arg-type]
    )


def test_yuv420_collator_materializes_views_and_preserves_other_fields() -> None:
    y = torch.arange(2 * 1 * 4 * 4, dtype=torch.uint8).reshape(2, 1, 4, 4)
    uv = torch.arange(2 * 2 * 2 * 2, dtype=torch.uint8).reshape(2, 2, 2, 2)
    frames = [
        Yuv420Frame(y=y[0], uv=uv[0], color_space="bt709", color_range="full"),
        Yuv420Frame(y=y[1], uv=uv[1], color_space="bt709", color_range="full"),
    ]

    stacked = Yuv420Frame.stack(frames)
    torch.testing.assert_close(stacked.y, y)
    torch.testing.assert_close(stacked.uv, uv)

    batch = Yuv420Collator()([
        {"video": frames[0], "state": torch.tensor([1.0])},
        {"video": frames[1], "state": torch.tensor([2.0])},
    ])

    assert isinstance(batch["video"], Yuv420Frame)
    torch.testing.assert_close(batch["video"].y, y)
    torch.testing.assert_close(batch["video"].uv, uv)
    torch.testing.assert_close(batch["state"], torch.tensor([[1.0], [2.0]]))
    frames[0].y.zero_()
    assert torch.count_nonzero(batch["video"].y[0]) > 0


def test_yuv420_frame_stack_rejects_empty_or_mixed_metadata() -> None:
    with pytest.raises(ValueError, match="empty sequence"):
        Yuv420Frame.stack([])

    limited = _frame(color_range="limited")
    full = _frame(color_range="full")
    with pytest.raises(ValueError, match="uniform color metadata"):
        Yuv420Frame.stack([limited, full])


@pytest.mark.parametrize(
    ("color_space", "color_range", "message"),
    [
        ("invalid", "full", "color space"),
        ("bt709", "invalid", "color range"),
    ],
)
def test_yuv420_frame_rejects_unsupported_metadata(color_space: str, color_range: str, message: str) -> None:
    with pytest.raises(ValueError, match=message):
        _frame(color_space=color_space, color_range=color_range)


def test_yuv420_to_rgb_warns_for_unspecified_metadata() -> None:
    frame = Yuv420Frame(
        y=torch.full((1, 2, 2), 16, dtype=torch.uint8),
        uv=torch.full((2, 1, 1), 128, dtype=torch.uint8),
        color_space="unspecified",
        color_range="unspecified",
    )

    with pytest.warns(UserWarning, match="defaulting color space to BT.601 and color range to limited"):
        converted = frame.to_rgb()

    torch.testing.assert_close(converted, torch.zeros((3, 2, 2)))


def test_yuv420_to_rgb_expands_each_chroma_sample_to_a_two_by_two_block() -> None:
    converted = _frame().to_rgb(normalize=False)

    torch.testing.assert_close(converted[:, 0, 0], converted[:, 0, 1])
    torch.testing.assert_close(converted[:, 0, 0], converted[:, 1, 0])
    assert not torch.equal(converted[:, 0, 1], converted[:, 0, 2])


def test_yuv420_cuda_conversion_rejects_dataloader_worker(monkeypatch: pytest.MonkeyPatch) -> None:
    import rerun.experimental.dataloader._yuv as yuv_module

    monkeypatch.setattr(yuv_module, "get_worker_info", lambda: object())

    with pytest.raises(RuntimeError, match="cannot run inside a DataLoader worker"):
        _frame().to_rgb("cuda")


def test_yuv420_types_are_pickleable_for_spawned_dataloader_workers() -> None:
    frame = _frame()
    restored = pickle.loads(pickle.dumps(frame))

    assert isinstance(restored, Yuv420Frame)
    torch.testing.assert_close(restored.y, frame.y)
    torch.testing.assert_close(restored.uv, frame.uv)
    assert isinstance(pickle.loads(pickle.dumps(Yuv420Collator())), Yuv420Collator)


def test_yuv420_collator_reports_missing_values() -> None:
    with pytest.raises(ValueError, match="filter or replace None"):
        Yuv420Collator()([{"video": _frame()}, {"video": None}])
