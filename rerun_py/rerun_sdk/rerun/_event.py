from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Literal

from typing_extensions import deprecated

# Type definitions based on `crates/viewer/re_viewer/src/event.rs`
# NOTE: In Python, we need to update both the type definitions
#       and the serialization code, which is in `viewer_event_from_json`.


@dataclass
class ViewerEventBase:
    """Base class for all viewer events."""

    application_id: str
    """The application ID of the recording that triggered the event."""

    recording_id: str
    """The recording ID of the recording that triggered the event."""

    segment_id: str | None
    """The segment ID of the recording that triggered the event, if any."""

    @property
    @deprecated("Use `segment_id` instead.")
    def partition_id(self) -> str | None:
        return self.segment_id


@dataclass
class EntitySelectionItem:
    """An entity selection item, representing a selected entity in the viewer."""

    @property
    def type(self) -> Literal["entity"]:
        return "entity"

    entity_path: str
    """The entity path of the selected entity."""

    instance_id: int | None = None
    """The instance ID of the selected entity, if any."""

    view_name: str | None = None
    """The name of the view containing the selected entity, if any."""

    position: list[float] | None = None
    """The 3D position of the selection, if any."""


@dataclass
class ViewSelectionItem:
    """A view selection item, representing a selected view in the viewer."""

    @property
    def type(self) -> Literal["view"]:
        return "view"

    view_id: str
    """The ID of the selected view."""

    view_name: str
    """The name of the selected view."""


@dataclass
class ContainerSelectionItem:
    """A container selection item, representing a selected container in the viewer."""

    @property
    def type(self) -> Literal["container"]:
        return "container"

    container_id: str
    """The ID of the selected container."""

    container_name: str
    """The name of the selected container."""


SelectionItem = EntitySelectionItem | ViewSelectionItem | ContainerSelectionItem
"""Union type for all possible selection item types."""


@dataclass
class PlayEvent(ViewerEventBase):
    """Event triggered when the viewer starts playing."""

    @property
    def type(self) -> Literal["play"]:
        return "play"


@dataclass
class PauseEvent(ViewerEventBase):
    """Event triggered when the viewer pauses playback."""

    @property
    def type(self) -> Literal["pause"]:
        return "pause"


@dataclass
class TimeUpdateEvent(ViewerEventBase):
    """Event triggered when the current time changes in the viewer."""

    @property
    def type(self) -> Literal["time_update"]:
        return "time_update"

    time: float
    """The new time value."""


@dataclass
class TimelineChangeEvent(ViewerEventBase):
    """Event triggered when the active timeline changes in the viewer."""

    @property
    def type(self) -> Literal["timeline_change"]:
        return "timeline_change"

    timeline: str
    """The name of the new active timeline."""

    time: float
    """The current time value on the new timeline."""


@dataclass
class SelectionChangeEvent(ViewerEventBase):
    """Event triggered when the selection changes in the viewer."""

    @property
    def type(self) -> Literal["selection_change"]:
        return "selection_change"

    items: list[SelectionItem]
    """The list of currently selected items."""


@dataclass
class RecordingOpenEvent(ViewerEventBase):
    """Event triggered when a recording is opened in the viewer."""

    @property
    def type(self) -> Literal["recording_open"]:
        return "recording_open"

    source: str
    """The source of the recording (e.g. a URL or file path)."""

    version: str | None
    """The version of the recording, if available."""


ViewerEvent = PlayEvent | PauseEvent | TimeUpdateEvent | TimelineChangeEvent | SelectionChangeEvent | RecordingOpenEvent
"""Union type for all possible viewer event types."""


def _viewer_event_from_json_str(json_str: str) -> ViewerEvent:
    data = json.loads(json_str)

    event_type: str = data["type"]
    app_id: str = data["application_id"]
    recording_id: str = data["recording_id"]
    segment_id: str | None = data.get("segment_id", None)

    if event_type == "play":
        return PlayEvent(application_id=app_id, recording_id=recording_id, segment_id=segment_id)

    elif event_type == "pause":
        return PauseEvent(application_id=app_id, recording_id=recording_id, segment_id=segment_id)

    elif event_type == "time_update":
        return TimeUpdateEvent(
            application_id=app_id,
            recording_id=recording_id,
            segment_id=segment_id,
            time=data["time"],
        )

    elif event_type == "timeline_change":
        return TimelineChangeEvent(
            application_id=app_id,
            recording_id=recording_id,
            segment_id=segment_id,
            timeline=data["timeline"],
            time=data["time"],
        )

    elif event_type == "selection_change":
        items: list[SelectionItem] = []
        for item in data["items"]:
            if item["type"] == "entity":
                items.append(
                    EntitySelectionItem(
                        entity_path=item["entity_path"],
                        instance_id=item.get("instance_id", None),
                        view_name=item.get("view_name", None),
                        position=item.get("position", None),
                    )
                )
            elif item["type"] == "view":
                items.append(
                    ViewSelectionItem(
                        view_id=item["view_id"],
                        view_name=item["view_name"],
                    )
                )
            elif item["type"] == "container":
                items.append(
                    ContainerSelectionItem(
                        container_id=item["container_id"],
                        container_name=item["container_name"],
                    )
                )

        return SelectionChangeEvent(
            application_id=app_id,
            recording_id=recording_id,
            segment_id=segment_id,
            items=items,
        )

    elif event_type == "recording_open":
        return RecordingOpenEvent(
            application_id=app_id,
            recording_id=recording_id,
            segment_id=segment_id,
            source=data["source"],
            version=data.get("version", None),
        )

    else:
        raise ValueError(f"Unknown event type: '{event_type}'")
