from __future__ import annotations

from .angle import override_angle__init_override
from .annotation_context import (
    override_annotation_info___native_to_pa_array_override,
    override_class_description___native_to_pa_array_override,
    override_class_description__init_override,
    override_class_description_info__field_converter_override,
    override_class_description_keypoint_annotations__field_converter_override,
    override_class_description_keypoint_connections__field_converter_override,
    override_class_description_map_elem___native_to_pa_array_override,
    override_keypoint_pair___native_to_pa_array_override,
)
from .class_id import override_class_id___native_to_pa_array_override
from .color import override_color___native_to_pa_array_override, override_color_rgba__field_converter_override
from .keypoint_id import override_keypoint_id___native_to_pa_array_override
from .matnxn import override_mat3x3_coeffs__field_converter_override, override_mat4x4_coeffs__field_converter_override
from .quaternion import override_quaternion__init_override
from .rotation3d import override_rotation3d__inner_converter_override
from .rotation_axis_angle import override_rotation_axis_angle_angle__field_converter_override
from .scale3d import override_scale3d__inner_converter_override
from .tensor_buffer import override_tensor_buffer__inner_converter_override
from .tensor_data import override_tensor_data___native_to_pa_array_override, override_tensor_data__init_override
from .transform3d import override_transform3d___native_to_pa_array_override
from .translation_and_mat3x3 import override_translation_and_mat3x3__init_override
from .translation_rotation_scale3d import override_translation_rotation_scale3d__init_override
from .utf8 import override_utf8___native_to_pa_array_override
from .vecxd import (
    override_vec2d___native_to_pa_array_override,
    override_vec3d___native_to_pa_array_override,
    override_vec4d___native_to_pa_array_override,
)
