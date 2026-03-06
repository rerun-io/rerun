use re_entity_db::EntityPath;

use crate::shader_params::ShaderParametersMeta;

/// Resolved shader parameters ready for GPU upload.
pub struct ResolvedShaderParams {
    /// Packed uniform buffer data (16-byte aligned per parameter).
    ///
    /// Each parameter is aligned to a 16-byte boundary. The WGSL uniform struct
    /// must include explicit padding fields to match this layout.
    pub uniform_data: Vec<u8>,

    /// Resolved 3D texture bindings: (`binding_index`, `entity_path`).
    pub texture_3d_bindings: Vec<(u32, EntityPath)>,
}

/// Resolve shader parameters into GPU-ready data.
///
/// For each uniform parameter, uses the provided `resolve_scalar` callback to
/// query the value. For textures, resolves entity paths.
pub fn resolve_shader_params(
    mesh_entity: &EntityPath,
    params: &ShaderParametersMeta,
    resolve_scalar: &dyn Fn(&EntityPath) -> Option<f64>,
    resolve_vec: &dyn Fn(&EntityPath, usize) -> Vec<f64>,
) -> ResolvedShaderParams {
    let buffer_size = params.uniform_buffer_size();
    let mut uniform_data = vec![0u8; buffer_size];
    let mut offset = 0usize;

    for uniform in &params.uniforms {
        // Align to 16 bytes
        offset = (offset + 15) & !15;

        let source_entity = resolve_entity_path(mesh_entity, &uniform.source);

        let param_size = match uniform.param_type.as_str() {
            "float" => {
                let value = resolve_scalar(&source_entity).unwrap_or(0.0) as f32;
                if offset + 4 <= uniform_data.len() {
                    uniform_data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
                4
            }
            "vec2" => {
                let values = resolve_vec(&source_entity, 2);
                write_float_values(&mut uniform_data, offset, &values, 2);
                8
            }
            "vec3" => {
                let values = resolve_vec(&source_entity, 3);
                write_float_values(&mut uniform_data, offset, &values, 3);
                12
            }
            "vec4" => {
                let values = resolve_vec(&source_entity, 4);
                write_float_values(&mut uniform_data, offset, &values, 4);
                16
            }
            "mat4" => {
                let values = resolve_vec(&source_entity, 16);
                write_float_values(&mut uniform_data, offset, &values, 16);
                64
            }
            other => {
                re_log::warn_once!("Unknown shader uniform type: {other}");
                4
            }
        };

        offset += param_size;
    }

    let texture_3d_bindings = params
        .textures
        .iter()
        .filter(|t| t.texture_type == "texture_3d")
        .map(|t| {
            let entity = resolve_entity_path(mesh_entity, &t.source);
            (t.binding, entity)
        })
        .collect();

    ResolvedShaderParams {
        uniform_data,
        texture_3d_bindings,
    }
}

fn write_float_values(buffer: &mut [u8], offset: usize, values: &[f64], count: usize) {
    for i in 0..count {
        let v = values.get(i).copied().unwrap_or(0.0) as f32;
        let start = offset + i * 4;
        let end = start + 4;
        if end <= buffer.len() {
            buffer[start..end].copy_from_slice(&v.to_le_bytes());
        }
    }
}

/// Resolve a potentially relative entity path.
///
/// Paths starting with "./" are resolved relative to `base_entity`.
/// All other paths are treated as absolute.
pub fn resolve_entity_path(base_entity: &EntityPath, source: &str) -> EntityPath {
    if let Some(relative) = source.strip_prefix("./") {
        let base = base_entity.to_string();
        EntityPath::parse_strict(&format!("{base}/{relative}")).unwrap_or_else(|_| {
            re_log::warn_once!("Failed to parse entity path: {base}/{relative}");
            base_entity.clone()
        })
    } else {
        EntityPath::parse_strict(source).unwrap_or_else(|_| {
            re_log::warn_once!("Failed to parse entity path: {source}");
            base_entity.clone()
        })
    }
}
