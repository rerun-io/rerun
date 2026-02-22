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
    # Stage 11: NativeArrowArray & build_fixed_size_list_array
    # ---------------------------------------------------------------
    print("\n--- Stage 11: NativeArrowArray & build_fixed_size_list_array ---")

    flat_vec3 = np.array([1.0, 2.0, 3.0], dtype=np.float32)
    flat_mat9 = np.ascontiguousarray(mat3x3_np.T.ravel())

    bench(
        "build_fixed_size_list_array(np, 3) [NativeArrowArray]",
        lambda: bindings.build_fixed_size_list_array(flat_vec3, 3),
    )

    bench(
        "build_fixed_size_list_array(np, 9) [NativeArrowArray]",
        lambda: bindings.build_fixed_size_list_array(flat_mat9, 9),
    )

    native_arr = bindings.build_fixed_size_list_array(flat_vec3, 3)
    bench("NativeArrowArray.to_pyarrow()", lambda: native_arr.to_pyarrow())

    bench("len(NativeArrowArray)", lambda: len(native_arr))

    bench("hasattr(transform, 'as_component_batches')", lambda: hasattr(transform, "as_component_batches"))

    bench("list(transform.as_component_batches())", lambda: list(transform.as_component_batches()))

    # _log_components directly (isolates decorator/glue overhead from rr.log)
    bench("_log_components(path, batches) [direct]", lambda: _log_components("test_entity", batches))

    # Trivial Python function call overhead
    bench("trivial_fn() call overhead", lambda: _trivial_fn())

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
