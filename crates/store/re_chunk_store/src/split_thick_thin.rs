//! Split chunks that mix "thick" columns (e.g. images, videos, blobs) and "thin" columns
//! (e.g. scalars, text, transforms) into separate chunks.
//!
//! The heuristic groups components by archetype, sorts those groups by byte size, and
//! splits wherever two neighbors differ by more than the given ratio. An archetype is
//! always kept together, components without an archetype are treated as a group of one.

use ahash::{HashMap, HashMapExt as _};
use itertools::Itertools as _;

use re_byte_size::SizeBytes as _;
use re_chunk::Chunk;
use re_types_core::{ArchetypeName, ComponentIdentifier};

/// How we group components before deciding where to split.
///
/// Components that belong to the same archetype always stay together. Components without
/// an archetype can be placed independently.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ComponentGroup {
    Archetype(ArchetypeName),
    Component(ComponentIdentifier),
}

/// Split a chunk so that no two groups sharing an output chunk differ in size by more than `ratio`.
///
/// Groups are sorted by byte size and split at every neighbor pair whose size ratio
/// meets or exceeds the threshold. A chunk with `k` such gaps becomes `k + 1` chunks.
///
/// Returns `None` if no split is needed.
pub(crate) fn split_chunk(chunk: &Chunk, ratio: f64) -> Option<Vec<Chunk>> {
    struct Group {
        bytes: u64,
        components: Vec<ComponentIdentifier>,
    }

    if chunk.components().len() < 2 {
        return None;
    }

    let mut groups: HashMap<ComponentGroup, Group> = HashMap::new();
    for column in chunk.components().values() {
        let key = match column.descriptor.archetype {
            Some(name) => ComponentGroup::Archetype(name),
            None => ComponentGroup::Component(column.descriptor.component),
        };
        let group = groups.entry(key).or_insert_with(|| Group {
            bytes: 0,
            components: Vec::new(),
        });
        group.bytes += column.heap_size_bytes();
        group.components.push(column.descriptor.component);
    }

    if groups.len() < 2 {
        return None;
    }

    let sorted: Vec<Group> = groups
        .into_values()
        .sorted_by_key(|g| std::cmp::Reverse(g.bytes))
        .collect();

    let mut split_points = Vec::new();
    for (i, window) in sorted.windows(2).enumerate() {
        let heavier = window[0].bytes as f64;
        let lighter = window[1].bytes.max(1) as f64;
        if heavier / lighter >= ratio {
            split_points.push(i + 1);
        }
    }

    if split_points.is_empty() {
        return None;
    }

    let mut splits = Vec::new();
    let mut start = 0;
    for end in split_points
        .iter()
        .copied()
        .chain(std::iter::once(sorted.len()))
    {
        let mut components = Vec::new();
        for group in &sorted[start..end] {
            components.extend_from_slice(&group.components);
        }

        // This will result in duplicate row ids since row ids are
        // preserved with `components_sliced`. Which is fine since
        // 1. We're not mutating the data the row contains.
        // 2. We're not splitting things in the same archetype.
        splits.push(chunk.components_sliced(&components));
        start = end;
    }

    re_log::debug!(
        entity = %chunk.entity_path(),
        num_groups = sorted.len(),
        num_splits = splits.len(),
        "splitting chunk on thick/thin boundaries"
    );

    Some(splits)
}

#[cfg(test)]
mod tests {
    use super::*;

    use re_chunk::RowId;
    use re_log_types::{EntityPath, Timeline, example_components::MyPoint};
    use re_sdk_types::components::Blob;
    use re_types_core::{ArchetypeName, ComponentDescriptor};

    #[test]
    fn splits_thick_from_thin() {
        re_log::setup_logging();

        let entity_path = EntityPath::from("mixed");
        let timepoint = [(Timeline::new_sequence("frame"), 1)];

        let points = &[MyPoint::new(1.0, 1.0)];
        let blob_bytes = 1024 * 128;
        let blob = Blob::from(vec![0u8; blob_bytes]);

        let points_descriptor = ComponentDescriptor {
            archetype: Some(ArchetypeName::from("my.archetype.Points")),
            component: "Points:points".into(),
            component_type: None,
        };
        let blob_descriptor = ComponentDescriptor {
            archetype: Some(ArchetypeName::from("my.archetype.Video")),
            component: "Video:blob".into(),
            component_type: None,
        };

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    (
                        points_descriptor,
                        points as &dyn re_types_core::ComponentBatch,
                    ),
                    (
                        blob_descriptor,
                        &[blob] as &dyn re_types_core::ComponentBatch,
                    ),
                ],
            )
            .build()
            .unwrap();

        let splits = split_chunk(&chunk, 10.0).expect("should split");
        assert_eq!(splits.len(), 2);
        let sizes: Vec<u64> = splits.iter().map(Chunk::heap_size_bytes).collect();
        assert!(
            sizes[0] > sizes[1] * 10,
            "expected thick split to dwarf thin split, got {sizes:?}"
        );
    }

    #[test]
    fn splits_three_tiers() {
        re_log::setup_logging();

        let entity_path = EntityPath::from("three_tiers");
        let timepoint = [(Timeline::new_sequence("frame"), 1)];

        let points = &[MyPoint::new(1.0, 1.0)];
        let medium = Blob::from(vec![0u8; 4 * 1024]);
        let heavy = Blob::from(vec![0u8; 512 * 1024]);

        let small_descriptor = ComponentDescriptor {
            archetype: Some(ArchetypeName::from("my.Points")),
            component: "Points:pos".into(),
            component_type: None,
        };
        let medium_descriptor = ComponentDescriptor {
            archetype: Some(ArchetypeName::from("my.Image")),
            component: "Image:blob".into(),
            component_type: None,
        };
        let heavy_descriptor = ComponentDescriptor {
            archetype: Some(ArchetypeName::from("my.Video")),
            component: "Video:blob".into(),
            component_type: None,
        };

        let chunk = Chunk::builder(entity_path)
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    (
                        small_descriptor,
                        points as &dyn re_types_core::ComponentBatch,
                    ),
                    (
                        medium_descriptor,
                        &[medium] as &dyn re_types_core::ComponentBatch,
                    ),
                    (
                        heavy_descriptor,
                        &[heavy] as &dyn re_types_core::ComponentBatch,
                    ),
                ],
            )
            .build()
            .unwrap();

        let splits = split_chunk(&chunk, 10.0).expect("should split");
        assert_eq!(
            splits.len(),
            3,
            "three clearly-separated tiers produce three chunks"
        );

        let sizes: Vec<u64> = splits.iter().map(Chunk::heap_size_bytes).collect();
        assert!(
            sizes[0] > sizes[1] && sizes[1] > sizes[2],
            "splits come out sorted heaviest-first, got {sizes:?}"
        );
    }

    #[test]
    fn leaves_uniform_chunk_alone() {
        re_log::setup_logging();

        let entity_path = EntityPath::from("balanced");
        let timepoint = [(Timeline::new_sequence("frame"), 1)];

        let p1 = &[MyPoint::new(1.0, 1.0)];
        let p2 = &[MyPoint::new(2.0, 2.0)];

        let chunk = Chunk::builder(entity_path)
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    (
                        ComponentDescriptor {
                            archetype: Some(ArchetypeName::from("my.Points")),
                            component: "Points:a".into(),
                            component_type: None,
                        },
                        p1 as &dyn re_types_core::ComponentBatch,
                    ),
                    (
                        ComponentDescriptor {
                            archetype: Some(ArchetypeName::from("my.Points")),
                            component: "Points:b".into(),
                            component_type: None,
                        },
                        p2 as &dyn re_types_core::ComponentBatch,
                    ),
                ],
            )
            .build()
            .unwrap();

        assert!(split_chunk(&chunk, 10.0).is_none());
    }
}
