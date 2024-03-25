// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::archetypes::Mesh3D> for CachedLatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<re_types::archetypes::Mesh3D> {
        re_tracing::profile_function!(<re_types::archetypes::Mesh3D>::name());

        // --- Required ---

        use re_types::components::Position3D;
        let vertex_positions = match self.get_required(<Position3D>::name()) {
            Ok(vertex_positions) => vertex_positions,
            Err(err) => return PromiseResult::Error(Arc::new(err)),
        };
        let vertex_positions = match vertex_positions.to_dense::<Position3D>(resolver).flatten() {
            PromiseResult::Ready(data) => data.to_vec(),
            PromiseResult::Pending => return PromiseResult::Pending,
            PromiseResult::Error(err) => return PromiseResult::Error(err),
        };

        // --- Recommended/Optional ---

        use re_types::components::MeshProperties;
        let mesh_properties = if let Some(mesh_properties) = self.get(<MeshProperties>::name()) {
            match mesh_properties
                .to_dense::<MeshProperties>(resolver)
                .flatten()
            {
                PromiseResult::Ready(data) => data.first().cloned(),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Vector3D;
        let vertex_normals = if let Some(vertex_normals) = self.get(<Vector3D>::name()) {
            match vertex_normals.to_dense::<Vector3D>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Color;
        let vertex_colors = if let Some(vertex_colors) = self.get(<Color>::name()) {
            match vertex_colors.to_dense::<Color>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Texcoord2D;
        let vertex_texcoords = if let Some(vertex_texcoords) = self.get(<Texcoord2D>::name()) {
            match vertex_texcoords.to_dense::<Texcoord2D>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Material;
        let mesh_material = if let Some(mesh_material) = self.get(<Material>::name()) {
            match mesh_material.to_dense::<Material>(resolver).flatten() {
                PromiseResult::Ready(data) => data.first().cloned(),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::TensorData;
        let albedo_texture = if let Some(albedo_texture) = self.get(<TensorData>::name()) {
            match albedo_texture.to_dense::<TensorData>(resolver).flatten() {
                PromiseResult::Ready(data) => data.first().cloned(),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::ClassId;
        let class_ids = if let Some(class_ids) = self.get(<ClassId>::name()) {
            match class_ids.to_dense::<ClassId>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        // ---

        let arch = re_types::archetypes::Mesh3D {
            vertex_positions,
            mesh_properties,
            vertex_normals,
            vertex_colors,
            vertex_texcoords,
            mesh_material,
            albedo_texture,
            class_ids,
        };

        PromiseResult::Ready(arch)
    }
}
