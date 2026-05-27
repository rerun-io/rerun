from __future__ import annotations

import numpy as np
import pyarrow as pa
import rerun as rr

VOXEL_INDEX_ARROW_TYPE = pa.list_(pa.field("item", pa.int32(), nullable=False), 3)


def test_voxel_grid_map_list_construction() -> None:
    voxel_indices = [(-1, 0, 2), (2, 1, 3)]
    values = [0.1, 0.9]

    arch = rr.VoxelGridMap(
        voxel_indices,
        0.25,
        values=values,
        colors=[0xAA0000CC, 0x00BB00DD],
        translation=[1.0, 2.0, 3.0],
        opacity=0.5,
        value_range=[0.0, 1.0],
        colormap=rr.components.Colormap.Turbo,
    )

    assert arch.voxel_indices == rr.components.VoxelIndexBatch._converter(voxel_indices)
    assert arch.voxel_indices is not None
    assert arch.voxel_indices.as_arrow_array().type == VOXEL_INDEX_ARROW_TYPE
    assert arch.voxel_indices.as_arrow_array().to_pylist() == [[-1, 0, 2], [2, 1, 3]]
    assert arch.cell_size == rr.components.CellSizeBatch._converter(0.25)
    assert arch.values == rr.components.VoxelValueBatch._converter(values)
    assert arch.colors == rr.components.ColorBatch._converter([0xAA0000CC, 0x00BB00DD])
    assert arch.translation == rr.components.Translation3DBatch._converter([1.0, 2.0, 3.0])
    assert arch.opacity == rr.components.OpacityBatch._converter(0.5)
    assert arch.value_range == rr.components.ValueRangeBatch._converter([0.0, 1.0])
    assert arch.colormap == rr.components.ColormapBatch._converter(rr.components.Colormap.Turbo)


def test_voxel_grid_map_ndarray_construction() -> None:
    voxel_indices = np.array([[-1, 0, 2], [1, 0, 0], [1, 1, 0]], dtype=np.int32)
    values = np.array([0.0, 0.5, 1.0], dtype=np.float32)

    arch = rr.VoxelGridMap(voxel_indices, 0.5, values=values)

    assert arch.voxel_indices == rr.components.VoxelIndexBatch._converter(voxel_indices)
    assert arch.voxel_indices is not None
    assert arch.voxel_indices.as_arrow_array().type == VOXEL_INDEX_ARROW_TYPE
    assert arch.voxel_indices.as_arrow_array().to_pylist() == [[-1, 0, 2], [1, 0, 0], [1, 1, 0]]
    assert arch.cell_size == rr.components.CellSizeBatch._converter(np.float32(0.5))
    assert arch.values == rr.components.VoxelValueBatch._converter(values)
