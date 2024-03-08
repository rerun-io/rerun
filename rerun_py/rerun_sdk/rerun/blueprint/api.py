from __future__ import annotations

import itertools
import uuid

import rerun_bindings as bindings

from .._log import log
from ..recording_stream import RecordingStream
from .archetypes.container_blueprint import ContainerBlueprint
from .archetypes.space_view_blueprint import SpaceViewBlueprint
from .archetypes.space_view_contents import SpaceViewContents
from .archetypes.viewport_blueprint import ViewportBlueprint
from .components.container_kind import ContainerKind


class Container:
    def __init__(self, kind, contents):
        self.id = uuid.uuid4()
        self.kind = kind
        self.contents = contents

    def path(self):
        return f"container/{self.id}"

    def log(self, stream):
        for sub in self.contents:
            sub.log(stream)

        bp = ContainerBlueprint(
            container_kind=self.kind,
            contents=[sub.path() for sub in self.contents],
            col_shares=[1 for _ in self.contents],
            row_shares=[1 for _ in self.contents],
            visible=True,
        )

        log(self.path(), bp, recording=stream)

    def iter_space_views(self):
        return itertools.chain.from_iterable(sub.iter_space_views() for sub in self.contents)


class Horizontal(Container):
    def __init__(self, *contents):
        super().__init__(ContainerKind.Horizontal, contents)


class Vertical(Container):
    def __init__(self, *contents):
        super().__init__(ContainerKind.Vertical, contents)


class Tabs(Container):
    def __init__(self, *contents):
        super().__init__(ContainerKind.Tabs, contents)


class SpaceView:
    def __init__(self, class_identifier, origin, contents):
        self.id = uuid.uuid4()
        self.class_identifier = class_identifier
        self.origin = origin
        self.contents = contents

    def path(self):
        return f"space_view/{self.id}"

    def log(self, stream):
        contents_bp = SpaceViewContents(query=self.contents)
        log(self.path() + "/SpaceViewContents", contents_bp, recording=stream)

        bp = SpaceViewBlueprint(
            class_identifier=self.class_identifier,
            space_origin=self.origin,
        )

        log(self.path(), bp, recording=stream)

    def iter_space_views(self):
        return [self.id.bytes]


class Spatial3D(SpaceView):
    def __init__(self, origin="/", contents="/**"):
        super().__init__("3D", origin, contents)


class Spatial2D(SpaceView):
    def __init__(self, origin="/", contents="/**"):
        super().__init__("2D", origin, contents)


class Viewport:
    def __init__(self, root_container):
        self.root_container = root_container

    def path(self):
        return "viewport"

    def log(self, stream):
        self.root_container.log(stream)

        bp = ViewportBlueprint(
            space_views=list(self.root_container.iter_space_views()),
            root_container=self.root_container.id.bytes,
            auto_layout=False,
            auto_space_views=False,
        )

        log(self.path(), bp, recording=stream)

    def create_blueprint(self, application_id):
        blueprint_stream = RecordingStream(
            bindings.new_blueprint(
                application_id=application_id,
            )
        )

        stream_native = blueprint_stream.to_native()

        blueprint_stream.set_time_seconds("blueprint", 1)

        self.log(stream_native)

        return blueprint_stream.memory_recording()
