from __future__ import annotations

import uuid
import itertools
from numpy.random import default_rng

import rerun as rr
from rerun.blueprint.archetypes.container_blueprint import ContainerBlueprint
from rerun.blueprint.archetypes.space_view_blueprint import SpaceViewBlueprint
from rerun.blueprint.archetypes.space_view_contents import SpaceViewContents
from rerun.blueprint.archetypes.viewport_blueprint import ViewportBlueprint
from rerun.blueprint.components.container_kind import ContainerKind


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

        rr.log(self.path(), bp, recording=stream)

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
        rr.log(self.path() + "/SpaceViewContents", contents_bp, recording=stream)

        bp = SpaceViewBlueprint(
            class_identifier=self.class_identifier,
            space_origin=self.origin,
        )

        rr.log(self.path(), bp, recording=stream)

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

        rr.log(self.path(), bp, recording=stream)


def create_blueprint(viewport: Viewport):
    stream = rr.experimental.new_blueprint("rerun_example_blueprint_test")
    stream.connect()
    stream_native = stream.to_native()

    rr.set_time_seconds("blueprint", 1, recording=stream)

    viewport.log(stream_native)


if __name__ == "__main__":
    rng = default_rng(12345)
    positions = rng.uniform(-5, 5, size=[10, 3])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.init("rerun_example_blueprint_test", spawn=True)
    rr.log("test1", rr.Points3D(positions, colors=colors, radii=radii))
    rr.log("test2", rr.Points2D(positions[:, :2], colors=colors, radii=radii))
    root = Vertical(
        Spatial3D(origin="/test1"),
        Horizontal(
            Tabs(
                Spatial3D(origin="/test1"),
                Spatial2D(origin="/test2"),
            ),
            Spatial2D(origin="/test2"),
        ),
    )
    viewport = Viewport(root)
    create_blueprint(viewport)
