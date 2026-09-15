"""
Tests for rerun.experimental.LeRobotReader.

Wrapper-plumbing tests against the committed importer fixtures
(`crates/data_flow/re_importer/tests/assets/lerobot/`). The conversion logic itself
is covered by `re_lerobot`'s Rust tests and the importer snapshot tests.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import pytest
from rerun.experimental import LeRobotReader

if TYPE_CHECKING:
    from rerun.chunk import Chunk

REPO_ROOT = Path(__file__).resolve().parents[3]
LEROBOT_ASSETS = REPO_ROOT / "crates" / "data_flow" / "re_importer" / "tests" / "assets" / "lerobot"

# From the fixture's `meta/episodes`.
V3_EPISODE_LENGTHS = [299, 300, 300]


def test_lerobot_reader_opens_and_reports_metadata() -> None:
    reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")
    assert reader.version == "v3"
    assert reader.episodes() == [0, 1, 2]

    reader = LeRobotReader(LEROBOT_ASSETS / "v21_apple_storage")
    assert reader.version == "v2"
    assert reader.episodes() == [0, 1, 2]


def test_lerobot_reader_rejects_bad_input(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        LeRobotReader(tmp_path / "does_not_exist")
    with pytest.raises(ValueError, match="LeRobot"):
        LeRobotReader(tmp_path)

    reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")
    with pytest.raises(ValueError, match="episode"):
        reader.stream(99, video_mode="skip")
    with pytest.raises(ValueError, match="video mode"):
        reader.stream(0, video_mode="bogus")  # type: ignore[arg-type]


def test_lerobot_stream_covers_exactly_each_episodes_rows() -> None:
    reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")

    for episode, expected_len in zip(reader.episodes(), V3_EPISODE_LENGTHS, strict=True):
        chunks = reader.stream(episode, video_mode="skip").to_chunks()
        by_entity = {chunk.entity_path: chunk for chunk in chunks if not chunk.is_static}

        assert set(by_entity) >= {"/action", "/observation.state", "/task"}
        assert by_entity["/action"].num_rows == expected_len
        assert by_entity["/action"].timeline_names == ["frame_index"]
        # Scalar features are rewrapped as one Scalars archetype component, so the chunk
        # holds exactly three columns: row id, frame_index, and the scalars.
        assert by_entity["/action"].num_columns == 3
        assert by_entity["/observation.state"].num_rows == expected_len


def test_lerobot_stream_options_reach_the_output() -> None:
    reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")

    chunks = reader.stream(0, entity_path_prefix="/robot", video_mode="skip").to_chunks()
    entities = {chunk.entity_path for chunk in chunks}
    assert "/robot/action" in entities
    assert all(entity.startswith("/robot") for entity in entities)

    chunks = reader.stream(0, timeline="my_time", video_mode="skip").to_chunks()
    timelines = {name for chunk in chunks for name in chunk.timeline_names}
    assert timelines == {"my_time"}


def test_lerobot_stream_twice_agrees() -> None:
    def structure(chunks: list[Chunk]) -> list[tuple[str, int, bool]]:
        return [(chunk.entity_path, chunk.num_rows, chunk.is_static) for chunk in chunks]

    reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")
    stream = reader.stream(0, video_mode="skip")

    first = structure(stream.to_chunks())
    second = structure(stream.to_chunks())
    assert first, "the stream must produce chunks"
    assert first == second, "re-executing one stream must produce an identical result"

    # A same-reader comparison alone cannot catch instance-dependent ordering, so also
    # compare against an independently opened reader.
    fresh_reader = LeRobotReader(LEROBOT_ASSETS / "v30_apple_storage")
    fresh = structure(fresh_reader.stream(0, video_mode="skip").to_chunks())
    assert first == fresh, "an independent reader must produce an identical result"
