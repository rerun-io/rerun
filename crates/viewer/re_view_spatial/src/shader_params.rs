//! Shader parameter metadata parsed from the JSON in `ShaderParameters`.
//!
//! Describes uniform parameters, their types, and the entity paths where
//! the data should be queried from the store.

/// A single uniform parameter.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UniformParam {
    /// Name of the uniform in the shader (e.g., `"density_scale"`).
    #[expect(dead_code)]
    pub name: String,

    /// Type of the uniform. One of: `"float"`, `"vec2"`, `"vec3"`, `"vec4"`, `"mat4"`.
    #[serde(rename = "type")]
    pub param_type: String,

    /// Relative or absolute entity path where the value should be queried.
    /// Relative paths are resolved relative to the `Mesh3D` entity.
    pub source: String,
}

/// A texture binding parameter.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TextureParam {
    /// Name of the texture in the shader (e.g., `"volume_data"`).
    #[expect(dead_code)]
    pub name: String,

    /// Type of texture binding. One of: `"texture_2d"`, `"texture_3d"`.
    #[serde(rename = "type")]
    pub texture_type: String,

    /// Binding index in the custom bind group.
    pub binding: u32,

    /// Entity path where the texture data should be queried.
    pub source: String,
}

/// Top-level shader parameters metadata.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ShaderParametersMeta {
    /// Uniform parameters (scalars, vectors, matrices).
    #[serde(default)]
    pub uniforms: Vec<UniformParam>,

    /// Texture bindings.
    #[serde(default)]
    pub textures: Vec<TextureParam>,
}

impl ShaderParametersMeta {
    /// Parse shader parameters from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Total number of bytes needed for the uniform buffer.
    ///
    /// Uses a simplified layout where each parameter occupies a 16-byte aligned
    /// slot. This means the WGSL uniform struct must use explicit padding to
    /// match (e.g., `_pad` fields between smaller types). This is intentionally
    /// conservative to avoid subtle cross-platform alignment bugs.
    ///
    /// Example: a `float` followed by a `vec2` uses 32 bytes total:
    /// - float at offset 0 (4 bytes, padded to 16)
    /// - vec2 at offset 16 (8 bytes, total padded to 32)
    pub fn uniform_buffer_size(&self) -> usize {
        let mut size = 0usize;
        for uniform in &self.uniforms {
            let param_size = match uniform.param_type.as_str() {
                "float" => 4,
                "vec2" => 8,
                "vec3" => 12,
                "vec4" => 16,
                "mat4" => 64,
                _ => {
                    re_log::warn_once!("Unknown shader uniform type: {}", uniform.param_type);
                    4
                }
            };
            // Align each parameter to 16 bytes (vec4 alignment).
            // The WGSL struct must include explicit padding to match this layout.
            size = (size + 15) & !15;
            size += param_size;
        }
        // Pad total to 16-byte alignment
        (size + 15) & !15
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shader_params() {
        let json = r#"{
            "uniforms": [
                { "name": "density_scale", "type": "float", "source": "./density_scale" },
                { "name": "volume_min", "type": "vec3", "source": "./volume_bounds" },
                { "name": "value_range", "type": "vec2", "source": "./value_range" }
            ],
            "textures": [
                { "name": "volume_data", "type": "texture_3d", "binding": 0, "source": "./volume_data" }
            ]
        }"#;

        let params = ShaderParametersMeta::from_json(json).unwrap();
        assert_eq!(params.uniforms.len(), 3);
        assert_eq!(params.textures.len(), 1);
        assert_eq!(params.uniforms[0].name, "density_scale");
        assert_eq!(params.uniforms[0].param_type, "float");
        assert_eq!(params.textures[0].texture_type, "texture_3d");
    }

    #[test]
    fn test_uniform_buffer_size() {
        let params = ShaderParametersMeta {
            uniforms: vec![
                UniformParam {
                    name: "density".into(),
                    param_type: "float".into(),
                    source: "./density".into(),
                },
                UniformParam {
                    name: "range".into(),
                    param_type: "vec2".into(),
                    source: "./range".into(),
                },
            ],
            textures: vec![],
        };

        // float (4 bytes) padded to 16, then vec2 (8 bytes) => 16 + 8 = 24, padded to 32
        assert_eq!(params.uniform_buffer_size(), 32);
    }

    #[test]
    fn test_empty_params() {
        let json = "{}";
        let params = ShaderParametersMeta::from_json(json).unwrap();
        assert!(params.uniforms.is_empty());
        assert!(params.textures.is_empty());
        assert_eq!(params.uniform_buffer_size(), 0);
    }
}
