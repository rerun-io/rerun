use std::collections::HashMap;
use three_d::core::*;
use three_d::renderer::*;

pub struct InstancedSpheres<M> {
    /// The geometry
    pub geometry: InstancedSperesGeom,

    /// The material applied to the geometry
    pub material: M,
}

impl<M: Material> InstancedSpheres<M> {
    pub fn new_with_material(
        context: &Context,
        instances: &SphereInstances,
        cpu_mesh: &CpuMesh,
        material: M,
    ) -> ThreeDResult<Self> {
        Ok(Self {
            geometry: InstancedSperesGeom::new(context, instances, cpu_mesh)?,
            material,
        })
    }
}

impl<M: Material> Geometry for InstancedSpheres<M> {
    fn aabb(&self) -> AxisAlignedBoundingBox {
        self.geometry.aabb()
    }

    fn render_with_material(
        &self,
        material: &dyn Material,
        camera: &Camera,
        lights: &[&dyn Light],
    ) -> ThreeDResult<()> {
        self.geometry.render_with_material(material, camera, lights)
    }
}

impl<M: Material> Object for InstancedSpheres<M> {
    fn render(&self, camera: &Camera, lights: &[&dyn Light]) -> ThreeDResult<()> {
        self.render_with_material(&self.material, camera, lights)
    }

    fn is_transparent(&self) -> bool {
        self.material.is_transparent()
    }
}

impl<M: Material> std::ops::Deref for InstancedSpheres<M> {
    type Target = InstancedSperesGeom;
    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

impl<M: Material> std::ops::DerefMut for InstancedSpheres<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.geometry
    }
}

// ----------------------------------------------------------------------------

pub struct InstancedSperesGeom {
    context: Context,
    vertex_buffers: HashMap<String, VertexBuffer>,
    instance_buffers: HashMap<String, InstanceBuffer>,
    index_buffer: Option<ElementBuffer>,
    aabb_local: AxisAlignedBoundingBox,
    aabb: AxisAlignedBoundingBox,
    transformation: Mat4,
    instance_transforms: Vec<Mat4>,
    instance_count: u32,
    texture_transform: Mat3,
}

impl InstancedSperesGeom {
    ///
    /// Creates a new 3D mesh from the given [CpuMesh].
    /// All data in the [CpuMesh] is transfered to the GPU, so make sure to remove all unnecessary data from the [CpuMesh] before calling this method.
    /// The mesh is rendered in as many instances as there are [Instance] structs given as input.
    /// The transformation and texture transform in [Instance] are applied to each instance before they are rendered.
    ///
    pub fn new(
        context: &Context,
        instances: &SphereInstances,
        cpu_mesh: &CpuMesh,
    ) -> ThreeDResult<Self> {
        let aabb = cpu_mesh.compute_aabb();
        let mut model = Self {
            context: context.clone(),
            index_buffer: index_buffer_from_mesh(context, cpu_mesh)?,
            vertex_buffers: vertex_buffers_from_mesh(context, cpu_mesh)?,
            instance_buffers: HashMap::new(),
            aabb,
            aabb_local: aabb.clone(),
            transformation: Mat4::identity(),
            instance_count: 0,
            instance_transforms: Vec::new(),
            texture_transform: Mat3::identity(),
        };
        model.set_instances(instances)?;
        Ok(model)
    }

    /// Returns the number of instances that is rendered.
    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }

    ///
    /// Update the instances.
    ///
    pub fn set_instances(&mut self, instances: &SphereInstances) -> ThreeDResult<()> {
        crate::profile_function!();
        #[cfg(debug_assertions)]
        instances.validate()?;
        self.instance_count = instances.count();
        self.instance_buffers.clear();
        self.instance_transforms = {
            crate::profile_scope!("Compute transforms");
            (0..self.instance_count as usize)
                .map(|i| {
                    Mat4::from_translation(instances.translations[i])
                        * instances
                            .rotations
                            .as_ref()
                            .map(|r| Mat4::from(r[i]))
                            .unwrap_or(Mat4::identity())
                        * instances
                            .scales
                            .as_ref()
                            .map(|s| Mat4::from_nonuniform_scale(s[i].x, s[i].y, s[i].z))
                            .unwrap_or(Mat4::identity())
                })
                .collect::<Vec<_>>()
        };

        if instances.rotations.is_none() && instances.scales.is_none() {
            self.instance_buffers.insert(
                "instance_translation".to_string(),
                InstanceBuffer::new_with_data(&self.context, &instances.translations)?,
            );
        } else {
            let mut row1 = Vec::new();
            let mut row2 = Vec::new();
            let mut row3 = Vec::new();
            for geometry_transform in self.instance_transforms.iter() {
                row1.push(vec4(
                    geometry_transform.x.x,
                    geometry_transform.y.x,
                    geometry_transform.z.x,
                    geometry_transform.w.x,
                ));

                row2.push(vec4(
                    geometry_transform.x.y,
                    geometry_transform.y.y,
                    geometry_transform.z.y,
                    geometry_transform.w.y,
                ));

                row3.push(vec4(
                    geometry_transform.x.z,
                    geometry_transform.y.z,
                    geometry_transform.z.z,
                    geometry_transform.w.z,
                ));
            }

            self.instance_buffers.insert(
                "row1".to_string(),
                InstanceBuffer::new_with_data(&self.context, &row1)?,
            );
            self.instance_buffers.insert(
                "row2".to_string(),
                InstanceBuffer::new_with_data(&self.context, &row2)?,
            );
            self.instance_buffers.insert(
                "row3".to_string(),
                InstanceBuffer::new_with_data(&self.context, &row3)?,
            );
        }

        if let Some(instance_colors) = &instances.colors {
            self.instance_buffers.insert(
                "instance_color".to_string(),
                InstanceBuffer::new_with_data(&self.context, &instance_colors)?,
            );
        }
        self.update_aabb();
        Ok(())
    }

    fn update_aabb(&mut self) {
        crate::profile_function!();
        let mut aabb = AxisAlignedBoundingBox::EMPTY;
        for i in 0..self.instance_count as usize {
            let mut aabb2 = self.aabb_local.clone();
            aabb2.transform(&(self.instance_transforms[i] * self.transformation));
            aabb.expand_with_aabb(&aabb2);
        }
        self.aabb = aabb;
    }

    fn vertex_shader_source(&self, fragment_shader_source: &str) -> ThreeDResult<String> {
        crate::profile_function!();
        let use_positions = fragment_shader_source.find("in vec3 pos;").is_some();
        let use_normals = fragment_shader_source.find("in vec3 nor;").is_some();
        let use_tangents = fragment_shader_source.find("in vec3 tang;").is_some();
        let use_uvs = fragment_shader_source.find("in vec2 uvs;").is_some();
        let use_colors = fragment_shader_source.find("in vec4 col;").is_some();
        Ok(format!(
            "{}{}{}{}{}{}{}{}",
            if self.instance_buffers.contains_key("instance_translation") {
                "#define USE_INSTANCE_TRANSLATIONS\n"
            } else {
                "#define USE_INSTANCE_TRANSFORMS\n"
            },
            if use_positions {
                "#define USE_POSITIONS\n"
            } else {
                ""
            },
            if use_normals {
                "#define USE_NORMALS\n"
            } else {
                ""
            },
            if use_tangents {
                if fragment_shader_source.find("in vec3 bitang;").is_none() {
                    Err(CoreError::MissingBitangent)?;
                }
                "#define USE_TANGENTS\n"
            } else {
                ""
            },
            if use_uvs { "#define USE_UVS\n" } else { "" },
            if use_colors {
                if self.instance_buffers.contains_key("instance_color")
                    && self.vertex_buffers.contains_key("color")
                {
                    "#define USE_COLORS\n#define USE_VERTEX_COLORS\n#define USE_INSTANCE_COLORS\n"
                } else if self.instance_buffers.contains_key("instance_color") {
                    "#define USE_COLORS\n#define USE_INSTANCE_COLORS\n"
                } else {
                    "#define USE_COLORS\n#define USE_VERTEX_COLORS\n"
                }
            } else {
                ""
            },
            r#"
#define PI 3.1415926

// clamping to 0 - 1 range
float saturate(in float value)
{
    return clamp(value, 0.0, 1.0);
}

vec3 srgb_from_rgb(vec3 rgb) {
	vec3 a = vec3(0.055, 0.055, 0.055);
	vec3 ap1 = vec3(1.0, 1.0, 1.0) + a;
	vec3 g = vec3(2.4, 2.4, 2.4);
	vec3 ginv = 1.0 / g;
	vec3 select = step(vec3(0.0031308, 0.0031308, 0.0031308), rgb);
	vec3 lo = rgb * 12.92;
	vec3 hi = ap1 * pow(rgb, ginv) - a;
	return mix(lo, hi, select);
}

vec3 rgb_from_srgb(vec3 srgb) {
	vec3 a = vec3(0.055, 0.055, 0.055);
	vec3 ap1 = vec3(1.0, 1.0, 1.0) + a;
	vec3 g = vec3(2.4, 2.4, 2.4);
	vec3 select = step(vec3(0.04045, 0.04045, 0.04045), srgb);
	vec3 lo = srgb / 12.92;
	vec3 hi = pow((srgb + a) / ap1, g);
	return mix(lo, hi, select);
}

vec3 world_pos_from_depth(mat4 viewProjectionInverse, float depth, vec2 uv) {
    vec4 clipSpacePosition = vec4(uv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
    vec4 position = viewProjectionInverse * clipSpacePosition;
    return position.xyz / position.w;
}

vec3 reinhard_tone_mapping(vec3 color) {
    return color / (color + vec3(1.0));
}

// http://holger.dammertz.org/stuff/notes_HammersleyOnHemisphere.html
// efficient VanDerCorpus calculation.
float RadicalInverse_VdC(uint bits)
{
     bits = (bits << 16u) | (bits >> 16u);
     bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
     bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
     bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
     bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
     return float(bits) * 2.3283064365386963e-10; // / 0x100000000
}

vec2 Hammersley(uint i, uint N)
{
	return vec2(float(i)/float(N), RadicalInverse_VdC(i));
}
            "#,
            r#"

uniform mat4 viewProjection;
uniform mat4 modelMatrix;
in vec3 position;

#ifdef USE_INSTANCE_TRANSLATIONS
in vec3 instance_translation;
#endif

#ifdef USE_INSTANCE_TRANSFORMS
in vec4 row1;
in vec4 row2;
in vec4 row3;
#endif

#ifdef USE_POSITIONS
out vec3 pos;
#endif

#ifdef USE_NORMALS
uniform mat4 normalMatrix;
in vec3 normal;
out vec3 nor;

#ifdef USE_TANGENTS
in vec4 tangent;
out vec3 tang;
out vec3 bitang;
#endif

#endif


#ifdef USE_UVS
uniform mat3 textureTransform;
in vec2 uv_coordinates;
out vec2 uvs;
#endif

#ifdef USE_VERTEX_COLORS
in vec4 color;
#endif
#ifdef USE_INSTANCE_COLORS
in vec4 instance_color;
#endif
#ifdef USE_COLORS
out vec4 col;
#endif

void main()
{
    mat4 local2World = modelMatrix;

#ifdef USE_INSTANCE_TRANSFORMS
    mat4 transform;
    transform[0] = vec4(row1.x, row2.x, row3.x, 0.0);
    transform[1] = vec4(row1.y, row2.y, row3.y, 0.0);
    transform[2] = vec4(row1.z, row2.z, row3.z, 0.0);
    transform[3] = vec4(row1.w, row2.w, row3.w, 1.0);
    local2World *= transform;
#endif

    vec4 worldPosition = local2World * vec4(position, 1.);
#ifdef USE_INSTANCE_TRANSLATIONS
    worldPosition.xyz += instance_translation;
#endif
    gl_Position = viewProjection * worldPosition;

#ifdef USE_POSITIONS
    pos = worldPosition.xyz;
#endif

#ifdef USE_NORMALS
#ifdef USE_INSTANCE_TRANSFORMS
    mat3 normalMat = mat3(transpose(inverse(local2World)));
#else
    mat3 normalMat = mat3(normalMatrix);
#endif
    nor = normalize(normalMat * normal);

#ifdef USE_TANGENTS
    tang = normalize(normalMat * tangent.xyz);
    bitang = normalize(cross(nor, tang) * tangent.w);
#endif

#endif

#ifdef USE_UVS
    mat3 texTransform = textureTransform;
    uvs = (texTransform * vec3(uv_coordinates, 1.0)).xy;
#endif


#ifdef USE_COLORS
    col = vec4(1.0, 1.0, 1.0, 1.0);
#ifdef USE_VERTEX_COLORS
    col *= color / 255.0;
#endif
#ifdef USE_INSTANCE_COLORS
    col *= instance_color / 255.0;
#endif
#endif
}
            "#,
        ))
    }
}

impl Geometry for InstancedSperesGeom {
    fn aabb(&self) -> AxisAlignedBoundingBox {
        self.aabb
    }

    fn render_with_material(
        &self,
        material: &dyn Material,
        camera: &Camera,
        lights: &[&dyn Light],
    ) -> ThreeDResult<()> {
        crate::profile_function!();
        let fragment_shader_source = material.fragment_shader_source(
            self.vertex_buffers.contains_key("color")
                || self.instance_buffers.contains_key("instance_color"),
            lights,
        );
        self.context.program(
            &self.vertex_shader_source(&fragment_shader_source)?,
            &fragment_shader_source,
            |program| {
                crate::profile_scope!("rendering");
                material.use_uniforms(program, camera, lights)?;
                program.use_uniform("viewProjection", camera.projection() * camera.view())?;
                program.use_uniform("modelMatrix", &self.transformation)?;
                program.use_uniform_if_required("textureTransform", &self.texture_transform)?;
                program.use_uniform_if_required(
                    "normalMatrix",
                    &self.transformation.invert().unwrap().transpose(),
                )?;

                for attribute_name in ["position", "normal", "tangent", "color", "uv_coordinates"] {
                    if program.requires_attribute(attribute_name) {
                        program.use_vertex_attribute(
                            attribute_name,
                            self.vertex_buffers
                                .get(attribute_name)
                                .ok_or(CoreError::MissingMeshBuffer(attribute_name.to_string()))?,
                        )?;
                    }
                }

                for attribute_name in [
                    "instance_translation",
                    "row1",
                    "row2",
                    "row3",
                    "instance_color",
                ] {
                    if program.requires_attribute(attribute_name) {
                        program.use_instance_attribute(
                            attribute_name,
                            self.instance_buffers
                                .get(attribute_name)
                                .ok_or(CoreError::MissingMeshBuffer(attribute_name.to_string()))?,
                        )?;
                    }
                }

                if let Some(ref index_buffer) = self.index_buffer {
                    program.draw_elements_instanced(
                        material.render_states(),
                        camera.viewport(),
                        index_buffer,
                        self.instance_count,
                    )
                } else {
                    program.draw_arrays_instanced(
                        material.render_states(),
                        camera.viewport(),
                        self.vertex_buffers.get("position").unwrap().vertex_count() as u32,
                        self.instance_count,
                    )
                }
            },
        )
    }
}

///
/// Defines the attributes for the instances of the model defined in [InstancedSperesGeom] or [InstancedModel].
/// Each list of attributes must contain the same number of elements as the number of instances.
///
#[derive(Clone, Debug, Default)]
pub struct SphereInstances {
    /// The translation applied to the positions of each instance.
    pub translations: Vec<Vec3>,
    /// The rotations applied to the positions of each instance.
    pub rotations: Option<Vec<Quat>>,
    /// The non-uniform scales applied to the positions of each instance.
    pub scales: Option<Vec<Vec3>>,
    /// Colors multiplied onto the base color of each instance.
    pub colors: Option<Vec<Color>>,
}

impl SphereInstances {
    ///
    /// Returns an error if the instances is not valid.
    ///
    pub fn validate(&self) -> ThreeDResult<()> {
        let instance_count = self.count();
        let buffer_check = |length: Option<usize>, name: &str| -> ThreeDResult<()> {
            if let Some(length) = length {
                if length < instance_count as usize {
                    Err(CoreError::InvalidBufferLength(
                        name.to_string(),
                        instance_count as usize,
                        length,
                    ))?;
                }
            }
            Ok(())
        };

        buffer_check(self.rotations.as_ref().map(|b| b.len()), "rotations")?;
        buffer_check(self.scales.as_ref().map(|b| b.len()), "scales")?;
        buffer_check(self.colors.as_ref().map(|b| b.len()), "colors")?;

        Ok(())
    }

    /// Returns the number of instances.
    pub fn count(&self) -> u32 {
        self.translations.len() as u32
    }
}

fn vertex_buffers_from_mesh(
    context: &Context,
    cpu_mesh: &CpuMesh,
) -> ThreeDResult<HashMap<String, VertexBuffer>> {
    #[cfg(debug_assertions)]
    cpu_mesh.validate()?;

    let mut buffers = HashMap::new();
    buffers.insert(
        "position".to_string(),
        VertexBuffer::new_with_data(context, &cpu_mesh.positions.to_f32())?,
    );
    if let Some(ref normals) = cpu_mesh.normals {
        buffers.insert(
            "normal".to_string(),
            VertexBuffer::new_with_data(context, normals)?,
        );
    };
    if let Some(ref tangents) = cpu_mesh.tangents {
        buffers.insert(
            "tangent".to_string(),
            VertexBuffer::new_with_data(context, tangents)?,
        );
    };
    if let Some(ref uvs) = cpu_mesh.uvs {
        buffers.insert(
            "uv_coordinates".to_string(),
            VertexBuffer::new_with_data(
                context,
                &uvs.iter()
                    .map(|uv| vec2(uv.x, 1.0 - uv.y))
                    .collect::<Vec<_>>(),
            )?,
        );
    };
    if let Some(ref colors) = cpu_mesh.colors {
        buffers.insert(
            "color".to_string(),
            VertexBuffer::new_with_data(context, colors)?,
        );
    };
    Ok(buffers)
}

fn index_buffer_from_mesh(
    context: &Context,
    cpu_mesh: &CpuMesh,
) -> ThreeDResult<Option<ElementBuffer>> {
    Ok(if let Some(ref indices) = cpu_mesh.indices {
        Some(match indices {
            Indices::U8(ind) => ElementBuffer::new_with_data(context, ind)?,
            Indices::U16(ind) => ElementBuffer::new_with_data(context, ind)?,
            Indices::U32(ind) => ElementBuffer::new_with_data(context, ind)?,
        })
    } else {
        None
    })
}
