//! Dataset-wide metadata, emitted as one static chunk, at [`DEFAULT_METADATA_ENTITY_PATH`] by default.
//!
//! Assets never contribute `/__properties` to a segment, so the metadata lives in its own
//! reserved namespace instead, where it cannot collide with feature entities either.

use std::path::Path;
use std::sync::Arc;

use arrow::array::StringArray;
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::AnyValues;

use crate::error::LeRobotError;

/// Where the metadata chunk lives unless the caller picks another entity path.
pub const DEFAULT_METADATA_ENTITY_PATH: &str = "/__lerobot";

/// The `meta/` files carried verbatim, each as a JSON string component named after its stem.
const METADATA_FILES: [&str; 2] = ["info", "stats"];

/// A static chunk at `entity_path` holding `meta/info.json` and, when present, `meta/stats.json`.
pub fn chunk(dataset_root: &Path, entity_path: EntityPath) -> Result<Chunk, LeRobotError> {
    let metadir = dataset_root.join("meta");
    let mut values = AnyValues::default();
    for name in METADATA_FILES {
        let path = metadir.join(format!("{name}.json"));
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|err| LeRobotError::io(err, &path))?;
        values = values.with_component_from_data(name, Arc::new(StringArray::from(vec![text])));
    }
    Ok(Chunk::builder(entity_path)
        .with_archetype(RowId::new(), TimePoint::STATIC, &values)
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_names(chunk: &Chunk) -> Vec<&str> {
        let mut names: Vec<_> = chunk.components().keys().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names
    }

    #[test]
    fn carries_only_the_files_present() {
        let dir = tempfile::tempdir().unwrap();
        let metadir = dir.path().join("meta");
        std::fs::create_dir(&metadir).unwrap();
        std::fs::write(metadir.join("info.json"), r#"{"fps": 30}"#).unwrap();

        let chunk = chunk(dir.path(), DEFAULT_METADATA_ENTITY_PATH.into()).unwrap();
        assert!(chunk.is_static());
        assert_eq!(
            chunk.entity_path(),
            &EntityPath::from(DEFAULT_METADATA_ENTITY_PATH)
        );
        assert_eq!(component_names(&chunk), ["info"]);

        std::fs::write(metadir.join("stats.json"), "{}").unwrap();
        assert_eq!(
            component_names(&super::chunk(dir.path(), "/custom/meta".into()).unwrap()),
            ["info", "stats"]
        );
    }
}
