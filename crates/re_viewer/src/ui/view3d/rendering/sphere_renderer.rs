//! A lot of this is copied from [`three-d`](https://github.com/asny/three-d).
//!
//! TODO(emilk): use billboards for each sphere instead to reduce vertex counts for large point clouds.

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
        instances: SphereInstances,
        cpu_mesh: &CpuMesh,
        material: M,
    ) -> Self {
        Self {
            geometry: InstancedSperesGeom::new(context, instances, cpu_mesh),
            material,
        }
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
    ) {
        self.geometry.render_with_material(material, camera, lights);
    }
}

impl<M: Material> Object for InstancedSpheres<M> {
    fn render(&self, camera: &Camera, lights: &[&dyn Light]) {
        self.render_with_material(&self.material, camera, lights);
    }

    fn material_type(&self) -> MaterialType {
        self.material.material_type()
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
    translation_scale_buffer: InstanceBuffer,
    color_buffer: InstanceBuffer,

    index_buffer: Option<ElementBuffer>,
    aabb: AxisAlignedBoundingBox,
    transformation: Mat4,
    instances: SphereInstances,
}

impl InstancedSperesGeom {
    pub fn new(context: &Context, instances: SphereInstances, cpu_mesh: &CpuMesh) -> Self {
        let mut model = Self {
            context: context.clone(),
            index_buffer: index_buffer_from_mesh(context, cpu_mesh),
            vertex_buffers: vertex_buffers_from_mesh(context, cpu_mesh),
            translation_scale_buffer: InstanceBuffer::new(context),
            color_buffer: InstanceBuffer::new(context),
            aabb: AxisAlignedBoundingBox::EMPTY,
            transformation: Mat4::identity(),
            instances: Default::default(),
        };
        model.set_instances(instances);
        model
    }

    pub fn instance_count(&self) -> u32 {
        self.instances.count()
    }

    pub fn set_instances(&mut self, instances: SphereInstances) {
        crate::profile_function!();
        instances.validate();

        self.translation_scale_buffer
            .fill(&instances.translations_and_scale);

        if let Some(instance_colors) = &instances.colors {
            self.color_buffer.fill(instance_colors);
        }
        self.instances = instances;
        self.update_aabb();
    }

    fn update_aabb(&mut self) {
        crate::profile_function!();

        let mut min = glam::Vec3A::splat(std::f32::INFINITY);
        let mut max = glam::Vec3A::splat(std::f32::NEG_INFINITY);

        for pos in &self.instances.translations_and_scale {
            let radius = glam::Vec3A::splat(pos.w);
            let pos = glam::Vec3A::new(pos.x, pos.y, pos.z);
            min = min.min(pos - radius);
            max = max.max(pos + radius);
        }
        self.aabb = AxisAlignedBoundingBox::new_with_positions(&[
            vec3(min.x, min.y, min.z),
            vec3(max.x, max.y, max.z),
        ]);
    }

    fn vertex_shader_source(&self, fragment_shader_source: &str) -> String {
        crate::profile_function!();
        let use_positions = fragment_shader_source.contains("in vec3 pos;");
        let use_normals = fragment_shader_source.contains("in vec3 nor;");
        let use_tangents = fragment_shader_source.contains("in vec3 tang;");
        let use_colors = fragment_shader_source.contains("in vec4 col;");
        format!(
            "{}{}{}{}{}",
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
                assert!(
                    !fragment_shader_source.contains("in vec3 bitang;"),
                    "Missing bitangent"
                );
                "#define USE_TANGENTS\n"
            } else {
                ""
            },
            if use_colors {
                if self.instances.colors.is_some() && self.vertex_buffers.contains_key("color") {
                    "#define USE_COLORS\n#define USE_VERTEX_COLORS\n#define USE_INSTANCE_COLORS\n"
                } else if self.instances.colors.is_some() {
                    "#define USE_COLORS\n#define USE_INSTANCE_COLORS\n"
                } else {
                    "#define USE_COLORS\n#define USE_VERTEX_COLORS\n"
                }
            } else {
                ""
            },
            r#"

uniform mat4 viewProjection;
uniform mat4 modelMatrix;
in vec3 position;

in vec4 instance_translation_scale;

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
    float scale = instance_translation_scale.w;
    vec4 worldPosition = modelMatrix * vec4(scale * position, 1.0);
    worldPosition.xyz += instance_translation_scale.xyz;
    gl_Position = viewProjection * worldPosition;

#ifdef USE_POSITIONS
    pos = worldPosition.xyz;
#endif

#ifdef USE_NORMALS
    mat3 normalMat = mat3(normalMatrix);
    nor = normalize(normalMat * normal);

    #ifdef USE_TANGENTS
        tang = normalize(normalMat * tangent.xyz);
        bitang = normalize(cross(nor, tang) * tangent.w);
    #endif
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
        )
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
    ) {
        crate::profile_function!();
        let fragment_shader_source = material.fragment_shader_source(
            self.vertex_buffers.contains_key("color") || self.instances.colors.is_some(),
            lights,
        );
        self.context
            .program(
                &self.vertex_shader_source(&fragment_shader_source),
                &fragment_shader_source,
                |program| {
                    crate::profile_scope!("rendering");
                    material.use_uniforms(program, camera, lights);
                    program.use_uniform("viewProjection", camera.projection() * camera.view());
                    program.use_uniform("modelMatrix", &self.transformation);
                    program.use_uniform_if_required(
                        "normalMatrix",
                        &self.transformation.invert().unwrap().transpose(),
                    );

                    for attribute_name in
                        ["position", "normal", "tangent", "color", "uv_coordinates"]
                    {
                        if program.requires_attribute(attribute_name) {
                            program.use_vertex_attribute(
                                attribute_name,
                                self.vertex_buffers
                                    .get(attribute_name)
                                    .unwrap_or_else(|| panic!("Missing {attribute_name:?}")),
                            );
                        }
                    }

                    if program.requires_attribute("instance_translation_scale") {
                        program.use_instance_attribute(
                            "instance_translation_scale",
                            &self.translation_scale_buffer,
                        );
                    }
                    if program.requires_attribute("instance_color") {
                        program.use_instance_attribute("instance_color", &self.color_buffer);
                    }

                    if let Some(ref index_buffer) = self.index_buffer {
                        program.draw_elements_instanced(
                            material.render_states(),
                            camera.viewport(),
                            index_buffer,
                            self.instances.count(),
                        );
                    } else {
                        program.draw_arrays_instanced(
                            material.render_states(),
                            camera.viewport(),
                            self.vertex_buffers.get("position").unwrap().vertex_count(),
                            self.instances.count(),
                        );
                    }
                },
            )
            .unwrap();
    }
}

#[derive(Clone, Debug, Default)]
pub struct SphereInstances {
    /// The translation applied to the positions of each instance.
    pub translations_and_scale: Vec<Vec4>,

    /// Colors multiplied onto the base color of each instance.
    pub colors: Option<Vec<Color>>,
}

impl SphereInstances {
    pub fn validate(&self) {
        if let Some(colors) = &self.colors {
            debug_assert_eq!(colors.len(), self.translations_and_scale.len());
        }
    }

    pub fn count(&self) -> u32 {
        self.translations_and_scale.len() as u32
    }
}

fn vertex_buffers_from_mesh(
    context: &Context,
    cpu_mesh: &CpuMesh,
) -> HashMap<String, VertexBuffer> {
    #[cfg(debug_assertions)]
    cpu_mesh.validate().unwrap();

    let mut buffers = HashMap::new();
    buffers.insert(
        "position".to_owned(),
        VertexBuffer::new_with_data(context, &cpu_mesh.positions.to_f32()),
    );
    if let Some(ref normals) = cpu_mesh.normals {
        buffers.insert(
            "normal".to_owned(),
            VertexBuffer::new_with_data(context, normals),
        );
    };
    if let Some(ref tangents) = cpu_mesh.tangents {
        buffers.insert(
            "tangent".to_owned(),
            VertexBuffer::new_with_data(context, tangents),
        );
    };
    if let Some(ref uvs) = cpu_mesh.uvs {
        buffers.insert(
            "uv_coordinates".to_owned(),
            VertexBuffer::new_with_data(
                context,
                &uvs.iter()
                    .map(|uv| vec2(uv.x, 1.0 - uv.y))
                    .collect::<Vec<_>>(),
            ),
        );
    };
    if let Some(ref colors) = cpu_mesh.colors {
        buffers.insert(
            "color".to_owned(),
            VertexBuffer::new_with_data(context, colors),
        );
    };
    buffers
}

fn index_buffer_from_mesh(context: &Context, cpu_mesh: &CpuMesh) -> Option<ElementBuffer> {
    cpu_mesh.indices.as_ref().map(|indices| match indices {
        Indices::U8(ind) => ElementBuffer::new_with_data(context, ind),
        Indices::U16(ind) => ElementBuffer::new_with_data(context, ind),
        Indices::U32(ind) => ElementBuffer::new_with_data(context, ind),
    })
}
