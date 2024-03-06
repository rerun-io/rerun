import uuid

import rerun as rr
from rerun.blueprint.archetypes.container_blueprint import ContainerBlueprint
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


class Horizontal(Container):
    def __init__(self, *contents):
        super().__init__(ContainerKind.Horizontal, contents)


class Vertical(Container):
    def __init__(self, *contents):
        super().__init__(ContainerKind.Vertical, contents)


class Viewport:
    def __init__(self, root_container):
        self.root_container = root_container

    def path(self):
        return "viewport"

    def log(self, stream):
        self.root_container.log(stream)

        bp = ViewportBlueprint(
            space_views=[],
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
    rr.init("rerun_example_blueprint_test", spawn=True)
    rr.log("test", rr.Points3D([1, 2, 3]))
    root = Horizontal(
        Vertical(),
        Vertical(),
    )
    viewport = Viewport(root)
    create_blueprint(viewport)
