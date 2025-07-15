from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Literal, Union

# Type definitions based on `crates/viewer/re_viewer/src/event.rs`
# NOTE: In Python, we need to update both the type definitions
#       and the serialization code, which is in `viewer_event_from_json`.


# Base class for all viewer events
@dataclass
class ViewerEventBase:
    application_id: str
    recording_id: str
    partition_id: str | None


# Selection item types with proper type discrimination
@dataclass
class EntitySelectionItem:
    @property
    def type(self) -> Literal["entity"]:
        return "entity"

    entity_path: str
    instance_id: int | None = None
    view_name: str | None = None
    position: list[float] | None = None


@dataclass
class ViewSelectionItem:
    @property
    def type(self) -> Literal["view"]:
        return "view"

    view_id: str
    view_name: str


@dataclass
class ContainerSelectionItem:
    @property
    def type(self) -> Literal["container"]:
        return "container"

    container_id: str
    container_name: str


SelectionItem = Union[EntitySelectionItem, ViewSelectionItem, ContainerSelectionItem]


# Concrete event classes
@dataclass
class PlayEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["play"]:
        return "play"


@dataclass
class PauseEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["pause"]:
        return "pause"


@dataclass
class TimeUpdateEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["time_update"]:
        return "time_update"

    time: float


@dataclass
class TimelineChangeEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["timeline_change"]:
        return "timeline_change"

    timeline: str
    time: float


@dataclass
class SelectionChangeEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["selection_change"]:
        return "selection_change"

    items: list[SelectionItem]


@dataclass
class RecordingOpenEvent(ViewerEventBase):
    @property
    def type(self) -> Literal["recording_open"]:
        return "recording_open"

    source: str
    version: str | None


# Union type for all possible event types
ViewerEvent = Union[
    PlayEvent,
    PauseEvent,
    TimeUpdateEvent,
    TimelineChangeEvent,
    SelectionChangeEvent,
    RecordingOpenEvent,
]


def _viewer_event_from_json_str(json_str: str) -> ViewerEvent:
    data = json.loads(json_str)

    event_type: str = data["type"]
    app_id: str = data["application_id"]
    recording_id: str = data["recording_id"]
    partition_id: str | None = data.get("partition_id", None)

    if event_type == "play":
        return PlayEvent(application_id=app_id, recording_id=recording_id, partition_id=partition_id)

    elif event_type == "pause":
        return PauseEvent(application_id=app_id, recording_id=recording_id, partition_id=partition_id)

    elif event_type == "time_update":
        return TimeUpdateEvent(
            application_id=app_id,
            recording_id=recording_id,
            partition_id=partition_id,
            time=data["time"],
        )

    elif event_type == "timeline_change":
        return TimelineChangeEvent(
            application_id=app_id,
            recording_id=recording_id,
            partition_id=partition_id,
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
            partition_id=partition_id,
            items=items,
        )

    elif event_type == "recording_open":
        return RecordingOpenEvent(
            application_id=app_id,
            recording_id=recording_id,
            partition_id=partition_id,
            source=data["source"],
            version=data.get("version", None),
        )

    else:
        raise ValueError(f"Unknown event type: '{event_type}'")
