# Rerun Python SDK architecture

## Background

Rerun primarily logs components, which are pieces of data with well-defined memory layout and semantics. For example, the `Color` component is stored as a `uint32` and represent a sRGB, RGBA rgba32 information. Components are typically logged in array form.

In most cases, multiple components are needed to represent an object to be logged and displayed in the Rerun viewer. For example, a 3D point cloud might need a `Position3D` component with the coordinates, a `Colors` component with the rgba32s, and a `Label` component with the text labels.

We call an `Archetype` a well-define collection of component that represent a give type of high-level object understood by the Rerun viewer. For example, the `Points3D` archetype (note the plural form) includes the following components:
- `Position3D`: the point coordinates (note the singular form)
- `Color`: the rgba32 information, if any
- `Label`: the textual label, if any
- `Radii`: the radius of the point, if any
- etc.

Some complex components are build using combination of another type of object called "datatype". These objects have well-defined memory layout but typically lack semantics. For example, the `Vec3D` datatype is a size 3 array of `float32`, and can be used by various components (for example, the `Position3D` component use the `Vec3D` datatype).

The purpose of the Python SDK is to make it easy to build archetype-conforming data structures and log them for subsequent display and inspection using the Rerun viewer, through an easy-to-learn, Pythonic API. To that end, it exposes an API over the supported archetypes, as well as all the components and datatypes needed to build them. In addition, the SDK provides the `rr.log()` function to log any archetype-conforming object, and several support function for initialization, recording session management, etc.

The present document focuses on the construction of archetype-conforming objects and the `rr.log()` function.


## Object types

Each of the archetype, component, and datatype made available by the Python SDK is implemented by up to three different kinds of objects, which are described here.

#### Native objects (`ObjectName`)

TODO(ab): generation of `__str__`, `__int__`, `__float__`
TODO(ab): generation of `__array__` (can be overridden)

The Python SDK makes extensive use of the  [`attrs`](https://www.attrs.org) package, which helps to minimize the amount of code to be generated and helps with the implementation.

#### Arrow extension type objects (`ObjectNameType`)

TODO(ab)


#### Arrow extension array objects (`ObjectNameArrayType`)

TODO(ab):
- `from_similar()`
- impl never autogen'd, so hand-coded required

#### Typing aliases (`ObjectNameLike` and `ObjectNameArrayLike`)

TODO(ab)


## Code generation

Keeping the various SDKs in sync with the Rerun Viewer requires automation to be tractable.
The Python SDK is no exception, and large parts of its implementation is generated using the `re_sdk_types` and `re_types_builder` crates, based on the object definitions found in `crates/store/re_sdk_types/definitions` and the generation code found in `crates/build/re_types_builder/src/codegen/python.rs`.

#### Archetype

In terms of code generation, archetypes are the simplest object. They consist of a native object whose fields are the various components that make up the archetype. The components are stored in their Arrow extension array form, such that they are ready to be sent to the Rerun Viewer or saved to a `.rrd` file. The fields always use the respective component's extension array `from_similar()` method as converter.

The archetype native objects are the primary user-facing API of the Rerun SDK.

#### Components

Component's key role is to provide the serialization of user data into their read-to-log, serialized Arrow array form. As such, a component's Arrow extension array object, and it's `from_similar()` method, play a key role. By convention, components must be structs with one, and exactly one, field.

The code generator distinguishes between delegating and non-delegating components

Delegating components use a datatype as field type, and their Arrow extension array object delegate its implementation to the corresponding datatype's. As a result, their implementation is very minimal, and forgoes native objects and typing aliases. `Point2D` is an example of delegating component (it delegates to the `Point2D` datatype).

Non-delegating components use a native type as field type, such as a `float` or `int` instead of relying on a separate datatypes. As a result, their implementation much resembles that of datatypes as they must handle data serialization in addition to their semantic role. In particular, a native object and typing aliases are generated.

#### Datatypes

Datatypes primary concern is modelling well-defined data structures, including a nice user-facing construction API and support for Arrow serialization. All the object types described in the previous section are thus generated for components.

Contrary to archetypes and components, datatypes occasionally represent complex data structures, such as `Transform3D`, made of nested structs and unions. The latter, lacking an obvious counterpart in Python, calls for a specific treatment (see below for details).


## Extensions

Though a fully automatic generation of an SDK conforming to our stated goals of ease-of-use and Pythonic API is asymptotically feasible, the effort required is disproportionate. The code generator thus offers a number of hooks allowing parts of the SDK to be hand-coded to handle edge cases and fine-tune the user-facing API.

This section covers the available hooks.

### The TypeExt class

During codegen, each class looks for a file: `class_ext.py` in the same directory where the class
will be generated. For example `datatypes/rgba32_ext.py` is the extension file for the `Rgba32` datatype,
which can be found in `datatypes/rgba32.py`.

In this file you must define a class called `<Type>Ext`, which will be added as a mixin to the generated class.

Any methods defined in the extension class will be accessible via the generated class, so this can be a helpful way of adding
things such as custom constructors.

Additionally the extension class allows you to override `__init__()`, `__array__()`, `native_to_pa_array`, or the field converters for any of the generated fields.

#### Native object init method (`__init__()`)

By default, the generated class uses `attrs` to generate an `__init__()` method that accepts keyword arguments for each field.

However, if your extension class includes its own `__init__()` method, the generated class will be created with
`@define(init=False)` so that `__init__` will instead be called on the extension class.

The override implementation may make use of the `__attrs_init__()` function, which `attrs`
[generates](https://www.attrs.org/en/stable/init.html#custom-init) to call through to the generated `__init__()` method
that would otherwise have been generated.

Init method overrides are typically used when the default constructor provided by `attrs` doesn't provide a satisfactory API. See `datatypes/angle_ext.py` for an example.

#### Native object field converter (`fieldname__field_converter_override()`)

This hook enables native objects to accept flexible input for their fields while normalizing their content to a
well-defined type. As this is a key enabler of a Pythonic user-facing API, most fields should have a converter set. The
code generator attempts to provide meaningful default converter whenever possible

The extension can define a custom converter for any field by implementing a `staticmethod` on the extension class.
The function must match the pattern `<fieldname>__field_converter_override`. This will be added as the `converter`
argument to the corresponding field definition.

See `components/color_ext.py` for an example.

#### Native object Numpy conversion method (`__array__()`)

If an object can be natively converted to a Numpy array it should implement the `__array__()` method, which in turn
allows Numpy to automatically ingest instances of that class in a controlled way.

By default, this will be generated automatically for types which only contain a single field which is a Numpy array. However,
other types can still implement this method on the extension class directly, in which case the default implementation will
be skipped.

#### PyArrow array conversion method (`native_to_pa_array_override()`)

This hook is the primary means of providing serialization to Arrow data, which is required for any non-delegating component or datatypes used by delegating components.

## Design notes on code generation

### Logging with `rr.log`

TODO(ab): auto upcasting

### Union type handling

TODO(ab):
- *not* inheritance (arm type might be reused elsewhere)
- `inner: Union[x, y]`
- `kind` for disambiguation (e.g. `Angle`) (typically requires init override for clean API)


### Interplay between hooks and generated methods

Implementing a Pythonic API for a component or datatype sometime require a subtle interplay between various hand-coded overrides and auto-generated methods. The `Color` component is a good illustration:

- The `ColorExt.rgba__field_converter_override()` converter flexibly normalizes user input into a `int` RGBA storage.
- The auto-generated `__int__()` method facilitates the conversion of a `Color` instance into a `int`, which is recognized by Numpy array creation functions.
- The `ColorExt.native_to_pa_array()` exploits these capabilities of the `Color` native object to simplify its implementation (even though, in most cases, the user code will skip using actual `Color` instances).

### Converter must be functions

The `converter` attribute of [`attrs.field()`] can be any callable. Often, `lambda` would make for a concise and efficient converter implementation, but, unfortunately, mypy doesn't support anything else than regular functions and emits error otherwise. For this reason, the code generator always uses regular functions when generating default converter. This is done by either of the following means:

- using built-in function (e.g. `int()`, for non-nullable `int` fields);
- using one of the functions provided in `_converters.py` (e.g. `int_or_none()` for nullable `int` fields);
- locally generating a bespoke converter function (e.g. for field using datatypes, nullable or otherwise).


### Typing

TODO(ab):
- fully typed SDK is a goal
- both mypy and pyright
- tests (in `rerun_py/tests/unit/`) serve as typing test as well and should minimize use of `# noqa` and similar
- unfortunately, PyCharm [doesn't properly support converters](https://youtrack.jetbrains.com/issue/PY-34243/attr.s-attr.ibconverter...-type-hinting-false-positive).

## Internal/wrapper pattern

Experience shows that the pattern of using a pure-Python wrapper around pyo3-based internal object is successful. It allows:
- keeping the "accept anything" magic on the Python side;
- having simple, canonically typed methods for rust-based objects (making it more likely to benefit from pyo3 magic type conversions).

See `rerun.catalog.CatalogClient` for an example of this pattern.

To use this pattern:

- Create a Rust object named `PyMyObjectInternal` in `src`. Expose it with pyo3 as `MyObjectInternal`.
- Crate type stubs for that object in `rerun_bindings`. These stubs should have no/minimal docstrings, since the rust-side docstrings are the reference. They should have precise type annotations, though.
- For internal objects, prefer simple signatures, ideally with a single type per argument.
- Create a wrapping public class `MyObject` in `rerun_sdk/rerun`. It should have a single data member called `_internal`, holding the internal object instance.
