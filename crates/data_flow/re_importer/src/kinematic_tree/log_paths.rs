use re_chunk::{Chunk, ChunkBuilder, ChunkId, EntityPath, RowId, TimePoint};
use re_log_types::EntityPathPart;
use re_sdk_types::AsComponents;
use re_sdk_types::archetypes::Transform3D;

/// The entity paths where the different parts of a robot model are logged.
pub struct LogPaths {
    /// The model's name, under the entity path prefix if one is set.
    pub root: EntityPath,

    /// Root of the visual geometries.
    ///
    /// Visual and collision geometries live under separate roots, so that each set can be hidden at once.
    pub visual_root: EntityPath,

    /// Root of the collision geometries.
    pub collision_root: EntityPath,

    /// All static joint transforms go to one entity, because they use named frames.
    ///
    /// Not affected by the entity path prefix.
    pub transforms: EntityPath,
}

impl LogPaths {
    pub fn new(model_name: &str, entity_path_prefix: Option<EntityPath>) -> Self {
        let root =
            entity_path_prefix.unwrap_or_else(EntityPath::root) / EntityPathPart::new(model_name);
        Self {
            visual_root: root.clone() / EntityPathPart::new("visual_geometries"),
            collision_root: root.clone() / EntityPathPart::new("collision_geometries"),
            transforms: EntityPath::from_single_string("tf_static"),
            root,
        }
    }
}

/// Emits `archetype` as a chunk of its own.
pub fn emit_archetype(
    emit: &mut dyn FnMut(Chunk),
    entity_path: EntityPath,
    timepoint: &TimePoint,
    archetype: &impl AsComponents,
) -> anyhow::Result<()> {
    emit(
        ChunkBuilder::new(ChunkId::new(), entity_path)
            .with_archetype(RowId::new(), timepoint.clone(), archetype)
            .build()?,
    );
    Ok(())
}

/// Emits `transforms` as the rows of one static chunk.
///
/// They are static so that users can override them on any other entity, on any timeline.
pub fn emit_static_transforms_batch(
    emit: &mut dyn FnMut(Chunk),
    entity_path: &EntityPath,
    transforms: &[Transform3D],
) -> anyhow::Result<()> {
    let mut chunk = ChunkBuilder::new(ChunkId::new(), entity_path.clone());
    for transform in transforms {
        chunk = chunk.with_archetype(RowId::new(), TimePoint::STATIC, transform);
    }
    emit(chunk.build()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_transforms_path_is_unaffected_by_prefix() {
        let paths = LogPaths::new("test", Some(EntityPath::parse_forgiving("robots/left_arm")));

        assert_eq!(paths.root.to_string(), "/robots/left_arm/test");
        assert_eq!(paths.transforms.to_string(), "/tf_static");
    }
}
