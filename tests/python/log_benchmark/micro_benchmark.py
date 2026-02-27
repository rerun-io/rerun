"""
Micro-benchmarks that isolate each stage of the rr.log() hot path.

Run from the rerun/ directory:
    pixi run uvpy tests/python/log_benchmark/micro_benchmark.py
"""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Any

import numpy as np
import pyarrow as pa
import rerun as rr
import rerun_bindings as bindings
from rerun._baseclasses import DescribedComponentBatch
from rerun._log import _log_components
from rerun.components import Translation3DBatch, TransformMat3x3Batch
from rerun.error_utils import catch_and_log_exceptions
from rerun_bindings import ComponentDescriptor

N = 100_000


def bench(label: str, fn: Callable[[], Any], *, warmup: int = 1000, iters: int = N) -> None:
    """Run fn() `iters` times and print throughput."""
    for _ in range(warmup):
        fn()
    start = time.perf_counter()
    for _ in range(iters):
        fn()
    elapsed = time.perf_counter() - start
    rate = iters / elapsed
    us_per = elapsed / iters * 1e6
    print(f"  {label:55s} {rate:>10.0f}/s  ({us_per:>6.2f} us/call)")


def main() -> None:
    rr.init("rerun_example_micro_benchmark", spawn=False)
    rr.memory_recording()

    # Prepare test data
    translation_list = [1.0, 2.0, 3.0]
    translation_np = np.array([1.0, 2.0, 3.0], dtype=np.float32)
    mat3x3_np = np.eye(3, dtype=np.float32)

    # Pre-build objects for later stages
    transform = rr.Transform3D(translation=translation_list, mat3x3=mat3x3_np)
    batches = transform.as_component_batches()

    # Pre-build arrow arrays and descriptors for _log_components
    descriptors = [b.component_descriptor() for b in batches]
    arrow_arrays = [b.as_arrow_array() for b in batches]
    instanced = dict(zip(descriptors, arrow_arrays))

    # Pre-build a Translation3DBatch for arrow conversion
    trans_batch = Translation3DBatch(translation_list)
    TransformMat3x3Batch(mat3x3_np)

    print(f"\n=== Micro-benchmarks ({N:,} iterations each) ===\n")

    # ---------------------------------------------------------------
    # Stage 0: Overhead of the benchmark loop itself
    # ---------------------------------------------------------------
    print("--- Stage 0: Loop overhead ---")
    bench("bare loop (pass)", lambda: None)

    # ---------------------------------------------------------------
    # Stage 1: Transform3D construction
    # ---------------------------------------------------------------
    print("\n--- Stage 1: Transform3D construction ---")

    bench(
        "Transform3D(translation=list, mat3x3=np)",
        lambda: rr.Transform3D(translation=translation_list, mat3x3=mat3x3_np),
    )

    bench(
        "Transform3D(translation=np, mat3x3=np)", lambda: rr.Transform3D(translation=translation_np, mat3x3=mat3x3_np)
    )

    # ---------------------------------------------------------------
    # Stage 1a: Individual component batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 1a: Individual batch construction ---")

    bench("Translation3DBatch(list)", lambda: Translation3DBatch(translation_list))

    bench("Translation3DBatch(np.array)", lambda: Translation3DBatch(translation_np))

    bench("TransformMat3x3Batch(np.eye(3))", lambda: TransformMat3x3Batch(mat3x3_np))

    # ---------------------------------------------------------------
    # Stage 1b: Component batch internals — arrow array construction
    # ---------------------------------------------------------------
    print("\n--- Stage 1b: Arrow array construction (no batch wrapper) ---")

    # Vec3D arrow conversion directly
    vec3d_type = Translation3DBatch._ARROW_DATATYPE
    bench(
        "Vec3D: flat_np_float32 + pa.FixedSizeListArray",
        lambda: pa.FixedSizeListArray.from_arrays(
            np.ascontiguousarray(np.asarray(translation_list, dtype=np.float32).ravel()), type=vec3d_type
        ),
    )

    bench(
        "Vec3D: pa.FixedSizeListArray from pre-made np",
        lambda: pa.FixedSizeListArray.from_arrays(translation_np, type=vec3d_type),
    )

    # Mat3x3 arrow conversion directly
    mat_type = TransformMat3x3Batch._ARROW_DATATYPE
    flat_mat = np.ascontiguousarray(mat3x3_np.T.ravel())
    bench(
        "Mat3x3: pa.FixedSizeListArray from pre-made np",
        lambda: pa.FixedSizeListArray.from_arrays(flat_mat, type=mat_type),
    )

    bench(
        "Mat3x3: full conversion (asarray+reshape+transpose+ravel+from_arrays)",
        lambda: pa.FixedSizeListArray.from_arrays(
            np.ascontiguousarray(
                np.asarray(mat3x3_np, dtype=np.float32).reshape(-1, 3, 3).transpose(0, 2, 1).reshape(-1)
            ),
            type=mat_type,
        ),
    )

    # ---------------------------------------------------------------
    # Stage 1c: catch_and_log_exceptions overhead
    # ---------------------------------------------------------------
    print("\n--- Stage 1c: catch_and_log_exceptions overhead ---")

    bench("catch_and_log_exceptions (context mgr, no-op body)", lambda: _noop_with_catch())

    bench("try/finally (baseline for comparison)", lambda: _noop_try_finally())

    # ---------------------------------------------------------------
    # Stage 2: as_component_batches()
    # ---------------------------------------------------------------
    print("\n--- Stage 2: as_component_batches() ---")

    bench("transform.as_component_batches()", lambda: transform.as_component_batches())

    # ---------------------------------------------------------------
    # Stage 3: _log_components (Python side before Rust)
    # ---------------------------------------------------------------
    print("\n--- Stage 3: _log_components ---")

    bench("_log_components(path, batches)", lambda: _log_components("test_entity", batches))

    # ---------------------------------------------------------------
    # Stage 4: bindings.log_arrow_msg (Rust side)
    # ---------------------------------------------------------------
    print("\n--- Stage 4: bindings.log_arrow_msg (Rust FFI) ---")

    bench(
        "bindings.log_arrow_msg(path, components)",
        lambda: bindings.log_arrow_msg("test_entity", components=instanced, static_=False, recording=None),
    )

    # ---------------------------------------------------------------
    # Stage 5: Full rr.log() pipeline
    # ---------------------------------------------------------------
    print("\n--- Stage 5: Full rr.log() pipeline ---")

    bench("rr.log(path, pre-built transform)", lambda: rr.log("test_entity", transform))

    bench(
        "rr.log(path, Transform3D(…)",
        lambda: rr.log("test_entity", rr.Transform3D(translation=translation_list, mat3x3=mat3x3_np)),
    )

    # ---------------------------------------------------------------
    # Stage 6: set_time overhead
    # ---------------------------------------------------------------
    print("\n--- Stage 6: set_time overhead ---")

    bench("rr.set_time('frame', sequence=42)", lambda: rr.set_time("frame", sequence=42))

    # ---------------------------------------------------------------
    # Stage 7: Misc overhead
    # ---------------------------------------------------------------
    print("\n--- Stage 7: Miscellaneous ---")

    bench(
        "ComponentDescriptor('Transform3D:translation', …)",
        lambda: ComponentDescriptor(
            "Transform3D:translation",
            component_type="rerun.components.Translation3D",
            archetype="rerun.archetypes.Transform3D",
        ),
    )

    bench("DescribedComponentBatch(batch, descr)", lambda: DescribedComponentBatch(trans_batch, descriptors[0]))

    bench("np.asarray(list, float32)", lambda: np.asarray(translation_list, dtype=np.float32))

    bench("np.asarray(np_arr, float32) [no-op]", lambda: np.asarray(translation_np, dtype=np.float32))

    bench("attrs converter: Translation3DBatch._converter(None)", lambda: Translation3DBatch._converter(None))

    bench(
        "attrs converter: Translation3DBatch._converter(list)", lambda: Translation3DBatch._converter(translation_list)
    )

    # ---------------------------------------------------------------
    # Stage 8: Deep-dive into batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 8: Batch construction deep-dive ---")

    from rerun._validators import flat_np_float32_array_from_array_like

    bench(
        "flat_np_float32_array_from_array_like(list, 3)",
        lambda: flat_np_float32_array_from_array_like(translation_list, 3),
    )

    bench(
        "flat_np_float32_array_from_array_like(np, 3)", lambda: flat_np_float32_array_from_array_like(translation_np, 3)
    )

    # The full _native_to_pa_array path for Vec3D
    bench(
        "Translation3DBatch._native_to_pa_array(list, type)",
        lambda: Translation3DBatch._native_to_pa_array(translation_list, vec3d_type),
    )

    bench(
        "Translation3DBatch._native_to_pa_array(np, type)",
        lambda: Translation3DBatch._native_to_pa_array(translation_np, vec3d_type),
    )

    # BaseBatch.__init__ overhead (includes catch_and_log_exceptions + isinstance check + _native_to_pa_array)
    bench("BaseBatch.__init__ via Translation3DBatch(list)", lambda: Translation3DBatch(translation_list))

    # What does isinstance(data, pa.Array) cost?
    bench("isinstance(list, pa.Array)", lambda: isinstance(translation_list, pa.Array))

    bench("isinstance(pa_array, pa.Array)", lambda: isinstance(arrow_arrays[0], pa.Array))

    # ---------------------------------------------------------------
    # Stage 9: Deep-dive into _log_components
    # ---------------------------------------------------------------
    print("\n--- Stage 9: _log_components deep-dive ---")

    # Build the dict manually to see where time goes
    bench("build instanced dict from batches", lambda: _build_instanced_dict(batches))

    bench(
        "list comprehension: [b.component_descriptor() for b in batches]",
        lambda: [b.component_descriptor() for b in batches],
    )

    bench("list comprehension: [b.as_arrow_array() for b in batches]", lambda: [b.as_arrow_array() for b in batches])

    # ---------------------------------------------------------------
    # Stage 10: attrs __attrs_init__ overhead
    # ---------------------------------------------------------------
    print("\n--- Stage 10: attrs __attrs_init__ overhead ---")

    # What does calling __attrs_init__ with all Nones cost?
    # (This is what happens for the 6 unused fields in Transform3D)
    inst = rr.Transform3D.__new__(rr.Transform3D)
    bench(
        "__attrs_init__(all None except translation+mat3x3)",
        lambda: inst.__attrs_init__(
            translation=translation_list,
            rotation_axis_angle=None,
            quaternion=None,
            scale=None,
            mat3x3=mat3x3_np,
            relation=None,
            child_frame=None,
            parent_frame=None,
        ),
    )

    # Cost of 6 _converter(None) calls (the converters that return None for unused fields)
    bench(
        "6x _converter(None) calls",
        lambda: (
            Translation3DBatch._converter(None),
            Translation3DBatch._converter(None),
            Translation3DBatch._converter(None),
            Translation3DBatch._converter(None),
            Translation3DBatch._converter(None),
            Translation3DBatch._converter(None),
        ),
    )

    # ---------------------------------------------------------------
    # Stage 11: Full rr.log() for various archetypes
    # ---------------------------------------------------------------
    print("\n--- Stage 11: Full rr.log() for various archetypes ---")

    # --- Points3D (Vec3D: float32, list_size=3) ---
    points3d_positions = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
    points3d_np = np.array(points3d_positions, dtype=np.float32)
    points3d = rr.Points3D(positions=points3d_np)
    bench("rr.log(Points3D(2 points, pre-built))", lambda: rr.log("bench", points3d))
    bench(
        "rr.log(Points3D(2 points, from np))",
        lambda: rr.log("bench", rr.Points3D(positions=points3d_np)),
    )

    # --- Points2D (Vec2D: float32, list_size=2) ---
    points2d_np = np.array([[1.0, 2.0], [3.0, 4.0]], dtype=np.float32)
    points2d = rr.Points2D(positions=points2d_np)
    bench("rr.log(Points2D(2 points, pre-built))", lambda: rr.log("bench", points2d))
    bench(
        "rr.log(Points2D(2 points, from np))",
        lambda: rr.log("bench", rr.Points2D(positions=points2d_np)),
    )

    # --- Arrows3D (Vec3D: float32, list_size=3) ---
    arrows_origins = np.zeros((2, 3), dtype=np.float32)
    arrows_vectors = np.ones((2, 3), dtype=np.float32)
    arrows3d = rr.Arrows3D(origins=arrows_origins, vectors=arrows_vectors)
    bench("rr.log(Arrows3D(2 arrows, pre-built))", lambda: rr.log("bench", arrows3d))
    bench(
        "rr.log(Arrows3D(2 arrows, from np))",
        lambda: rr.log("bench", rr.Arrows3D(origins=arrows_origins, vectors=arrows_vectors)),
    )

    # --- Transform3D with quaternion (Quaternion: float32, list_size=4) ---
    quat_np = np.array([0.0, 0.0, 0.0, 1.0], dtype=np.float32)
    xform_quat = rr.Transform3D(translation=translation_np, quaternion=rr.Quaternion(xyzw=quat_np))
    bench("rr.log(Transform3D(trans+quat, pre-built))", lambda: rr.log("bench", xform_quat))
    bench(
        "rr.log(Transform3D(trans+quat, from np))",
        lambda: rr.log("bench", rr.Transform3D(translation=translation_np, quaternion=rr.Quaternion(xyzw=quat_np))),
    )

    # --- Transform3D with mat3x3 (Mat3x3: float32, list_size=9) ---
    bench("rr.log(Transform3D(trans+mat3x3, pre-built))", lambda: rr.log("bench", transform))
    bench(
        "rr.log(Transform3D(trans+mat3x3, from np))",
        lambda: rr.log("bench", rr.Transform3D(translation=translation_list, mat3x3=mat3x3_np)),
    )

    # --- Pinhole (Mat3x3 + Vec2D) ---
    pinhole = rr.Pinhole(focal_length=[500.0, 500.0], resolution=[1920, 1080], principal_point=[960.0, 540.0])
    bench("rr.log(Pinhole(pre-built))", lambda: rr.log("bench", pinhole))

    # --- LineStrips3D (Vec3D: float32, list_size=3) ---
    strip = np.array([[0, 0, 0], [1, 1, 1], [2, 0, 0]], dtype=np.float32)
    linestrips = rr.LineStrips3D([strip])
    bench("rr.log(LineStrips3D(1 strip, pre-built))", lambda: rr.log("bench", linestrips))

    # ---------------------------------------------------------------
    # Stage 12: Batch construction for each numpy-backed datatype
    # ---------------------------------------------------------------
    print("\n--- Stage 12: Batch construction per datatype ---")

    from rerun.components import (
        Position2DBatch,
        Position3DBatch,
        Vector3DBatch,
    )
    from rerun.datatypes import (
        DVec2DBatch,
        Mat3x3Batch,
        Mat4x4Batch,
        Plane3DBatch,
        QuaternionBatch,
        Range1DBatch,
        UVec2DBatch,
        UVec3DBatch,
        UuidBatch,
        Vec2DBatch,
        Vec3DBatch,
        Vec4DBatch,
        ViewCoordinatesBatch,
    )

    # float32 types
    bench("Vec2DBatch([1.0, 2.0])", lambda: Vec2DBatch([1.0, 2.0]))
    bench("Vec3DBatch([1.0, 2.0, 3.0])", lambda: Vec3DBatch([1.0, 2.0, 3.0]))
    bench("Vec4DBatch([1.0, 2.0, 3.0, 4.0])", lambda: Vec4DBatch([1.0, 2.0, 3.0, 4.0]))
    bench("QuaternionBatch(xyzw=[0,0,0,1])", lambda: QuaternionBatch([0.0, 0.0, 0.0, 1.0]))
    bench("Plane3DBatch([1,0,0,0])", lambda: Plane3DBatch([1.0, 0.0, 0.0, 0.0]))

    mat3_input = np.eye(3, dtype=np.float32)
    bench("Mat3x3Batch(np.eye(3))", lambda: Mat3x3Batch(mat3_input))

    mat4_input = np.eye(4, dtype=np.float32)
    bench("Mat4x4Batch(np.eye(4))", lambda: Mat4x4Batch(mat4_input))

    # float64 types
    bench("DVec2DBatch([1.0, 2.0])", lambda: DVec2DBatch([1.0, 2.0]))
    bench("Range1DBatch([0.0, 1.0])", lambda: Range1DBatch([0.0, 1.0]))

    # uint32 types
    bench("UVec2DBatch([1, 2])", lambda: UVec2DBatch([1, 2]))
    bench("UVec3DBatch([1, 2, 3])", lambda: UVec3DBatch([1, 2, 3]))

    # uint8 types
    uuid_data = list(range(16))
    bench("UuidBatch(range(16))", lambda: UuidBatch(uuid_data))

    bench("ViewCoordinatesBatch(rr.ViewCoordinates.RDF)", lambda: ViewCoordinatesBatch(rr.ViewCoordinates.RDF))

    # Component-level batches (same underlying types)
    pos3_np = np.array([1.0, 2.0, 3.0], dtype=np.float32)
    bench("Position3DBatch(np)", lambda: Position3DBatch(pos3_np))
    pos2_np = np.array([1.0, 2.0], dtype=np.float32)
    bench("Position2DBatch(np)", lambda: Position2DBatch(pos2_np))
    vec3_np = np.array([1.0, 0.0, 0.0], dtype=np.float32)
    bench("Vector3DBatch(np)", lambda: Vector3DBatch(vec3_np))

    # ---------------------------------------------------------------
    # Stage 13: Primitive scalar batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 13: Primitive scalar batch construction ---")

    from rerun.components import ClassIdBatch, ColorBatch, OpacityBatch, RadiusBatch, ScalarBatch, ShowLabelsBatch
    from rerun.datatypes import BoolBatch, Float32Batch, Float64Batch, UInt16Batch, UInt32Batch

    np_f32_100 = np.ones(100, dtype=np.float32)

    bench("Float32Batch(1.0)", lambda: Float32Batch(1.0))
    bench("Float32Batch(np 100)", lambda: Float32Batch(np_f32_100))
    bench("UInt16Batch(42)", lambda: UInt16Batch(42))
    bench("UInt32Batch(0xFF0000FF)", lambda: UInt32Batch(0xFF0000FF))
    bench("BoolBatch(True)", lambda: BoolBatch(True))
    bench("Float64Batch(1.0)", lambda: Float64Batch(1.0))
    bench("RadiusBatch(1.0)", lambda: RadiusBatch(1.0))
    bench("ColorBatch(0xFF0000FF)", lambda: ColorBatch(0xFF0000FF))
    bench("ClassIdBatch(42)", lambda: ClassIdBatch(42))
    bench("OpacityBatch(0.5)", lambda: OpacityBatch(0.5))
    bench("ShowLabelsBatch(True)", lambda: ShowLabelsBatch(True))
    bench("ScalarBatch(1.0)", lambda: ScalarBatch(1.0))

    # ---------------------------------------------------------------
    # Stage 13b: Enum batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 13b: Enum batch construction ---")

    from rerun.components import (
        AggregationPolicyBatch,
        ColormapBatch,
        FillModeBatch,
        MarkerShapeBatch,
    )

    bench("AggregationPolicyBatch('Average')", lambda: AggregationPolicyBatch("Average"))
    bench("ColormapBatch('Viridis')", lambda: ColormapBatch("Viridis"))
    bench("FillModeBatch('Solid')", lambda: FillModeBatch("Solid"))
    bench("MarkerShapeBatch('Circle')", lambda: MarkerShapeBatch("Circle"))

    # ---------------------------------------------------------------
    # Stage 13c: String and variable-length batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 13c: String and variable-length batch construction ---")

    from rerun.components import LineStrip2DBatch, LineStrip3DBatch
    from rerun.datatypes import BlobBatch, Utf8Batch

    bench("Utf8Batch('hello')", lambda: Utf8Batch("hello"))
    bench("Utf8Batch(['a', 'b', 'c'])", lambda: Utf8Batch(["a", "b", "c"]))

    strip3d_np = np.array([[0, 0, 0], [1, 1, 1], [2, 0, 0]], dtype=np.float32)
    bench("LineStrip3DBatch([3-point strip])", lambda: LineStrip3DBatch([strip3d_np]))

    strip2d_np = np.array([[0, 0], [1, 1], [2, 0]], dtype=np.float32)
    bench("LineStrip2DBatch([3-point strip])", lambda: LineStrip2DBatch([strip2d_np]))

    blob_data = np.zeros(1000, dtype=np.uint8)
    bench("BlobBatch(np 1000 bytes)", lambda: BlobBatch(blob_data))
    bench("BlobBatch(b'hello')", lambda: BlobBatch(b"hello"))

    # ---------------------------------------------------------------
    # Stage 13d: Struct batch construction
    # ---------------------------------------------------------------
    print("\n--- Stage 13d: Struct batch construction ---")

    from rerun.datatypes import (
        AnnotationInfo,
        AnnotationInfoBatch,
        ImageFormat,
        ImageFormatBatch,
        Range2D,
        Range2DBatch,
        RotationAxisAngle,
        RotationAxisAngleBatch,
        Utf8Pair,
        Utf8PairBatch,
    )

    raa = RotationAxisAngle(axis=[0.0, 0.0, 1.0], angle=1.57)
    bench("RotationAxisAngleBatch(single)", lambda: RotationAxisAngleBatch(raa))
    raa_list = [raa] * 10
    bench("RotationAxisAngleBatch(list of 10)", lambda: RotationAxisAngleBatch(raa_list))

    ai = AnnotationInfo(id=1, label="car")
    bench("AnnotationInfoBatch(single)", lambda: AnnotationInfoBatch(ai))
    ai_list = [ai] * 10
    bench("AnnotationInfoBatch(list of 10)", lambda: AnnotationInfoBatch(ai_list))

    up = Utf8Pair(first="key", second="value")
    bench("Utf8PairBatch(single)", lambda: Utf8PairBatch(up))
    up_list = [up] * 10
    bench("Utf8PairBatch(list of 10)", lambda: Utf8PairBatch(up_list))

    r2d = Range2D(x_range=[0.0, 1.0], y_range=[0.0, 1.0])
    bench("Range2DBatch(single)", lambda: Range2DBatch(r2d))

    imgfmt = ImageFormat(width=640, height=480)
    bench("ImageFormatBatch(single)", lambda: ImageFormatBatch(imgfmt))

    # ---------------------------------------------------------------
    # Stage 13e: Full rr.log() with struct-containing archetypes
    # ---------------------------------------------------------------
    print("\n--- Stage 13e: Full rr.log() with struct archetypes ---")

    from rerun.archetypes import AnnotationContext

    xform_raa = rr.Transform3D(rotation=raa)
    bench("rr.log(Transform3D(rotation=AxisAngle), pre-built)", lambda: rr.log("bench", xform_raa))
    bench(
        "rr.log(Transform3D(rotation=AxisAngle), from scratch)",
        lambda: rr.log("bench", rr.Transform3D(rotation=RotationAxisAngle(axis=[0, 0, 1], angle=1.57))),
    )

    ctx = AnnotationContext(class_descriptions=[(0, "background"), (1, "car")])
    bench("rr.log(AnnotationContext, pre-built)", lambda: rr.log("bench", ctx))

    # ---------------------------------------------------------------
    # Stage 14: Scalar-heavy archetype end-to-end
    # ---------------------------------------------------------------
    print("\n--- Stage 14: Scalar-heavy archetype end-to-end ---")

    np_positions = np.array([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], dtype=np.float32)
    points3d_full = rr.Points3D(positions=np_positions, colors=[255, 0, 0], radii=[0.1], class_ids=[42])
    bench("rr.log(Points3D(full, pre-built))", lambda: rr.log("bench", points3d_full))
    bench(
        "rr.log(Points3D(full, from scratch))",
        lambda: rr.log("bench", rr.Points3D(positions=np_positions, colors=[255, 0, 0], radii=[0.1], class_ids=[42])),
    )

    print()


def _build_instanced_dict(batches: list[DescribedComponentBatch]) -> dict[ComponentDescriptor, pa.Array]:
    instanced = {}
    for comp in batches:
        descr = comp.component_descriptor()
        array = comp.as_arrow_array()
        instanced[descr] = array
    return instanced


def _noop_with_catch() -> None:
    with catch_and_log_exceptions("test"):
        pass


def _noop_try_finally() -> None:
    try:
        pass
    finally:
        pass


def _trivial_fn() -> None:
    return None


if __name__ == "__main__":
    main()
