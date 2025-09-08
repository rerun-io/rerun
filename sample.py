from __future__ import annotations

import functools
from dataclasses import dataclass, fields
from typing import Any, Callable, TypedDict, TypeVar, Unpack

import rerun as rr
from rerun import AnyValues, DynamicArchetype

T = TypeVar("T", bound=type)


class DataClassKwargs(TypedDict):
    init: bool
    repr: bool
    eq: bool
    order: bool
    unsafe_hash: bool
    frozen: bool
    match_args: bool
    kw_only: bool
    slots: bool
    weakref_slot: bool


class RerunMetadata(TypedDict):
    archetype_name: str


def any_values(**kwargs: Unpack[DataClassKwargs]) -> Callable[..., T]:
    def decorator(cls: T) -> T:
        dataclass_version = dataclass(**kwargs)(cls)
        dataclass_version._any_values = AnyValues()

        dataclass_init = dataclass_version.__init__

        @functools.wraps(dataclass_init)
        def new_init(self, *args: Any, **kwargs: Any):
            dataclass_init(self, *args, **kwargs)
            dataclass_field_values = {}
            for field in fields(self):
                # TODO(nick) check if type is rerun type
                dataclass_field_values[field.name] = getattr(self, field.name)
            self._rerun_any_values = AnyValues(**dataclass_field_values)

        dataclass_version.__init__ = new_init
        dataclass_version.as_component_batches = lambda self: self._rerun_any_values.as_component_batches()
        return dataclass_version

    return decorator


def dynamic_archetype(**kwargs: Unpack[DataClassKwargs | RerunMetadata]) -> Callable[..., T]:
    archetype: str = kwargs.pop("archetype_name", "DynamicArchetype")

    def decorator(cls: T) -> T:
        dataclass_version = dataclass(**kwargs)(cls)
        dataclass_version._any_values = AnyValues()

        dataclass_init = dataclass_version.__init__

        @functools.wraps(dataclass_init)
        def new_init(self, *args: Any, **kwargs: Any):
            dataclass_init(self, *args, **kwargs)
            dataclass_field_values = {}
            for field in fields(self):
                # TODO(nick) check if type is rerun type
                dataclass_field_values[field.name] = getattr(self, field.name)
            self._rerun_archetype = DynamicArchetype(archetype=archetype, components=dataclass_field_values)

        dataclass_version.__init__ = new_init
        dataclass_version.as_component_batches = lambda self: self._rerun_archetype.as_component_batches()
        return dataclass_version

    return decorator


@dynamic_archetype(archetype_name="MyData")
class AnotherExample:
    value: int


@any_values(frozen=False)
class MyData:
    value: int


if __name__ == "__main__":
    print(MyData)
    data = MyData(value=10)
    print(data)
    rr.init("rerun_example_decorators")
    rr.save("here.rrd")
    rr.log("/sample", data)
    rr.log("/another", AnotherExample(value=12))
