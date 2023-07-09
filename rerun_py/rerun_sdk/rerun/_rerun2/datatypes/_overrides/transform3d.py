from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# TODO(#2623): lots of boilerplate here that could be auto-generated

# TODO(ab): WIP

# def translationrotationscale_native_to_pa_array(
#     data: TranslationRotationScale3DLike, data_type: pa.DataType
# ) -> pa.Array:
#     pass
#
#
# def translationandmat3x3_native_to_pa_array(data: TranslationAndMat3x3Like, data_type: pa.DataType) -> pa.Array:
#     pass
#
#
# def transform3d_native_to_pa_array(data: Transform3DLike, data_type: pa.DataType) -> pa.Array:
#     from .. import TranslationAndMat3x3, TranslationRotationScale3D
#
#     if isinstance(data, TranslationRotationScale3D):
#         pass
#     elif isinstance(data, TranslationAndMat3x3):
#         pass
#     else:
#         pass  # TODO(ab): how to deal with sequence
#
#     return pa.array(array, type=data_type)
