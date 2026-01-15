"""Data types for RRD to LeRobot conversion."""

from __future__ import annotations

from dataclasses import dataclass

from .utils import get_entity_path


@dataclass(frozen=True)
class ImageSpec:
    """Specification for a video stream in the dataset."""

    key: str
    path: str


@dataclass(frozen=True)
class ColumnSpec:
    """Column names for action, state, and task data."""

    action: str | None
    state: str | None
    task: str | None


@dataclass(frozen=True)
class ConversionConfig:
    """Configuration for converting RRD data to LeRobot format."""

    # Output configuration
    fps: int
    index_column: str

    # Column specifications
    columns: ColumnSpec

    # Image/video specifications
    image_specs: list[ImageSpec]
    use_videos: bool
    video_format: str
    vcodec: str

    # Feature names
    action_names: list[str] | None
    state_names: list[str] | None

    # Task configuration
    task_default: str

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

        if self.columns.action:
            entity_path = get_entity_path(self.columns.action)
            contents.append(entity_path)
            if reference_path is None:
                reference_path = entity_path

        if self.columns.state:
            entity_path = get_entity_path(self.columns.state)
            if entity_path not in contents:
                contents.append(entity_path)
            if reference_path is None:
                reference_path = entity_path

        if self.columns.task:
            entity_path = get_entity_path(self.columns.task)
            if entity_path not in contents:
                contents.append(entity_path)

        for spec in self.image_specs:
            if spec.path not in contents:
                contents.append(spec.path)

        return contents, reference_path
