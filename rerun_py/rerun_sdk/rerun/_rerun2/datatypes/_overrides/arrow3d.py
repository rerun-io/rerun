from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from .. import Arrow3DArrayLike


def arrow3d_native_to_pa_array(data: Arrow3DArrayLike, data_type: pa.DataType) -> pa.Array:
    from rerun.experimental import dt as rrd

    from .. import Arrow3D

    # TODO: not quite sure why i must unwrap `xyz` or face a cryptic error otherwise

    # WARNING:rerun:The input object of type 'Vec3D' is an array-like implementing one
    # of the corresponding protocols (`__array__`, `__array_interface__` or `__array_struct__`);
    # but not a sequence (or 0-D). In the future, this object will be coerced as if it was first
    # converted using `np.array(obj)`. To retain the old behaviour, you have to either modify the
    # type 'Vec3D', or assign to an empty array created with `np.empty(correct_shape, dtype=object)`.
    #
    # WARNING:rerun:Creating an ndarray from ragged nested sequences (which is a list-or-tuple of
    # lists-or-tuples-or ndarrays with different lengths or shapes) is deprecated.
    # If you meant to do this, you must specify 'dtype=object' when creating the ndarray.

    if isinstance(data, Arrow3D):
        origins = rrd.Vec3DArray.from_similar(data.origin.xyz).storage
        vectors = rrd.Vec3DArray.from_similar(data.vector.xyz).storage
    else:
        origins = rrd.Vec3DArray.from_similar([d.origin.xyz for d in data]).storage
        vectors = rrd.Vec3DArray.from_similar([d.vector.xyz for d in data]).storage

    return pa.StructArray.from_arrays(
        arrays=[origins, vectors],
        fields=list(data_type),
    )
