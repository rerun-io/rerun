"""Data types for RRD to LeRobot conversion."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, NotRequired, TypedDict

import numpy as np
import numpy.typing as npt

from rerun_export.utils import get_entity_path

if TYPE_CHECKING:
    import rerun as rr

VideoSampleData = tuple[list[bytes], npt.NDArray[np.int64]]


class FeatureSpec(TypedDict):
    """Typed feature specification for LeRobot datasets."""

    dtype: str
    shape: tuple[int, ...]
    names: list[str] | None


class RemuxInfo(TypedDict):
    """Typed remuxing details for a single video stream."""

    samples: list[bytes]
    times_ns: npt.NDArray[np.int64]
    source_fps: float


class RemuxData(TypedDict):
    """Typed remuxing payload passed between conversion steps."""

    specs: list[VideoSpec]
    remux_info: dict[str, RemuxInfo]
    fps: int


class VideoSpec(TypedDict):
    """Specification for a video stream in the dataset."""

    key: str
    path: str
    video_format: NotRequired[str]


@dataclass(frozen=True)
class LeRobotConversionConfig:
    """Configuration for converting RRD data to LeRobot format."""

    # Output configuration
    fps: int
    index_column: str

    # Column specifications
    action: str
    state: str
    task: str

    # Image/video specifications
    videos: list[VideoSpec]
    use_videos: bool = True

    # Feature names
    action_names: list[str] | None = None
    state_names: list[str] | None = None

    # Task configuration
    task_default: str = "task"

    dataset: rr.catalog.DatasetEntry | None = None
    segment_id: str | None = None

    def get_filter_list(self) -> tuple[list[str], str | None]:
        """
        Get the list of entity paths to filter and the reference path for time alignment.

        Returns:
            A tuple of (contents, reference_path) where:
            - contents: List of unique entity paths to include in the query
            - reference_path: The entity path to use as reference for time alignment (action or state)

        """
        contents = []
        reference_path = None

        entity_path = get_entity_path(self.action)
        contents.append(entity_path)
        if reference_path is None:
            reference_path = entity_path

        entity_path = get_entity_path(self.state)
        if entity_path not in contents:
            contents.append(entity_path)
        if reference_path is None:
            reference_path = entity_path

        if self.task:
            entity_path = get_entity_path(self.task)
            if entity_path not in contents:
                contents.append(entity_path)

        for spec in self.videos:
            if spec["path"] not in contents:
                contents.append(spec["path"])

        return contents, reference_path
