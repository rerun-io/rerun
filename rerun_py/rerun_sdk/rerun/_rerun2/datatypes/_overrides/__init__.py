from __future__ import annotations

from .angle import override_angle_init
from .annotation_context import (
    override_annotation_info_native_to_pa_array,
    override_class_description_info_converter,
    override_class_description_init,
    override_class_description_keypoint_annotations_converter,
    override_class_description_keypoint_connections_converter,
    override_class_description_native_to_pa_array,
    override_class_description_map_elem_native_to_pa_array,
    override_keypoint_pair_native_to_pa_array,
)
from .class_id import override_class_id_native_to_pa_array
from .color import override_color_native_to_pa_array, override_color_rgba_converter
from .keypoint_id import override_keypoint_id_native_to_pa_array
from .matnxn import override_mat3x3_coeffs_converter, override_mat4x4_coeffs_converter
from .quaternion import override_quaternion_init
from .rotation3d import override_rotation3d_inner_converter
from .rotation_axis_angle import override_rotation_axis_angle_angle_converter
from .scale3d import override_scale3d_inner_converter
from .tensor_buffer import override_tensor_buffer_inner_converter
from .tensor_data import override_tensor_data_init, override_tensor_data_native_to_pa_array
from .transform3d import override_transform3d_native_to_pa_array
from .translation_and_mat3x3 import override_translation_and_mat3x3_init
from .translation_rotation_scale3d import override_translation_rotation_scale3d_init
from .utf8 import override_utf8_native_to_pa_array
from .vecxd import (
    override_vec2d_native_to_pa_array,
    override_vec3d_native_to_pa_array,
    override_vec4d_native_to_pa_array,
)
