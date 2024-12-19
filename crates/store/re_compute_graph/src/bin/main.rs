// let mut sm = SlotMap::new();
// let foo = sm.insert("foo");  // Key generated on insert.
// let bar = sm.insert("bar");
// assert_eq!(sm[foo], "foo");
// assert_eq!(sm[bar], "bar");
//
// sm.remove(bar);
// let reuse = sm.insert("reuse");  // Space from bar reused.
// assert_eq!(sm.contains_key(bar), false);  // After deletion a key stays invalid.
//
// let mut sec = SecondaryMap::new();
// sec.insert(foo, "noun");  // We provide the key for secondary maps.
// sec.insert(reuse, "verb");
//
// for (key, val) in sm {
//     println!("{} is a {}", val, sec[key]);
// }

// TODO: I don't particularly enjoy that TypeMap impl.

use ahash::HashSet;
use slotmap::{SecondaryMap, SlotMap};
use type_map::concurrent::TypeMap;

// inputs, outputs, bootstraps (aka constant inputs)

// TODO: why though?
pub struct ComputeGraphContext<'a> {
    // TODO: could even be several i guess, or a ChunkStoreHub or something.
    store: &'a ChunkStore,
}

// TODO: I guess we can just autodetect output nodes, they have no dependees

#[derive(Default)]
pub struct ComputeGraph {
    // TODO: in the end im not sure we even need one
    nodes: SlotMap<slotmap::DefaultKey, Box<dyn ComputeNode>>,
    dependencies: SecondaryMap<slotmap::DefaultKey, HashSet<slotmap::DefaultKey>>,
}

impl ComputeGraph {
    pub fn add_edge(&mut self) {}

    // TODO: has to be `self` to it can be erased/dyn
    // TODO: we don't have a TypeSet lul
    pub fn inputs(&self) -> HashSet<NamedTypeId> {
        dbg!([NamedTypeId::new::<&ChunkStore>(),].into_iter().collect())
    }

    pub fn execute(&self, ctx: ComputeGraphContext<'_>) -> TypeMap {
        for (_, node) in self.nodes.iter() {
            //
        }
        Default::default()
        // ctx.store
        //     .latest_at_relevant_chunks(query, entity_path, component_name)
    }
}

// TODO: reminder, we want to re-use graphs as-is across different stores, different hubs,
// different queries, different everything. There should be *one* LRU for everything.
// TODO: a ComputeGraph implements ComputeNode, that's it!

pub trait ComputeNode {
    // TODO: can we have nice dedicated paths for the usual suspects though? Chunk and such?
    // TODO: fn inputs() -> Set<TypeId> ?
    // TODO: fn outputs() -> Map<TypeId, dyn Any> ?
    // TODO: fn cacheable?
}

pub struct Kek;

impl ComputeNode for Kek {}

// TODO:
// * materialize AggregatedLatestAt
// * materialize AggregatedRange
// * materialize Dataframe
// * materialize the stuff in `Caches`:
//     * ImageDecodeCache
//     * ImageStatsCache
//     * TensorStatsCache
//     * MeshCache
//     * WireframeCache (wut?)
//     * SolidCache (wut?)
//     * VideoCache
//     * TransformCache (but should be unrelated)

// TODO: AggregatedLatestAt
// 1. Perform store-level LatestAtRelevantChunks for list of components
// 2. Perform chunk-level LatestAt for each chunk/component pair

// TODO: we want *one* CPU LRU and *one* GPU LRU
// -> let's start with CPU though.

// TODO: there is no invalidation, nobody cares -- pure LRU

#[derive(Default)]
pub struct LatestAtRelevantChunks;

pub struct LatestAtRelevantChunksOutput {
    pub chunks: Vec<Arc<Chunk>>,
}

// TODO: Hash of a Chunk is ChunkId and we call it a day?

// TODO: graph definition vs. graph instantiation

#[derive(Debug)]
pub struct NamedTypeId(pub &'static str, pub TypeId);

impl PartialEq for NamedTypeId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.1.eq(&other.1)
    }
}

impl Eq for NamedTypeId {}

// TODO: can this be nohash?
impl std::hash::Hash for NamedTypeId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}

impl NamedTypeId {
    #[inline]
    pub fn new<T: 'static>() -> Self {
        Self(std::any::type_name::<T>(), std::any::TypeId::of::<T>())
    }
}

// TODO: how does one static assert object safety these days?

// TODO: what does this need?
// * ChunkStore
// * LatestAtQuery
// * EntityPath
// * ComponentName
impl LatestAtRelevantChunks {
    // TODO: we can move to a cow when we need to
    pub fn name(&self) -> &'static str {
        //
    }

    // TODO: has to be `self` to it can be erased/dyn
    // TODO: we don't have a TypeSet lul
    pub fn inputs(&self) -> HashSet<NamedTypeId> {
        dbg!([
            NamedTypeId::new::<&ChunkStore>(),
            NamedTypeId::new::<LatestAtQuery>(),
            NamedTypeId::new::<EntityPath>(),
            NamedTypeId::new::<ComponentName>(),
        ]
        .into_iter()
        .collect())
    }

    pub fn execute(&self, ctx: ComputeGraphContext<'_>) -> TypeMap {
        Default::default()
        // ctx.store
        //     .latest_at_relevant_chunks(query, entity_path, component_name)
    }
}

// ---

use std::{any::TypeId, collections::HashMap, sync::Arc};

use itertools::Itertools;
use re_chunk::{Chunk, ComponentName, EntityPath, LatestAtQuery, Timeline};
use re_chunk_store::{ChunkStore, ChunkStoreConfig};
use re_log_encoding::VersionPolicy;
use re_log_types::{EntityPathFilter, StoreKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect_vec();

    let get_arg = |i| {
        let Some(value) = args.get(i) else {
            let bin_name = args.first().map_or("$BIN", |s| s.as_str());
            eprintln!(
                "{}",
                unindent::unindent(&format!(
                    "\
                    Usage: {bin_name} <path_to_rrd>

                    You can use one of your recordings, or grab one from our hosted examples, e.g.:
                    {bin_name} <(curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o -)\
                    ",
                )),
            );
            std::process::exit(1);
        };
        value
    };

    let path_to_rrd = get_arg(1);
    let entity_path_filter = EntityPathFilter::try_from(args.get(2).map_or("/**", |s| s.as_str()))?;
    let timeline = Timeline::log_time();

    let stores = ChunkStore::from_rrd_filepath(
        &ChunkStoreConfig::DEFAULT,
        path_to_rrd,
        VersionPolicy::Warn,
    )?;

    for (store_id, store) in stores {
        if store_id.kind != StoreKind::Recording {
            continue;
        }

        LatestAtRelevantChunks.inputs();
    }

    Ok(())
}

pub fn latest_at(
    store: &ChunkStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    component_name: ComponentName,
) -> Option<Arc<Chunk>> {
    // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
    //re_tracing::profile_scope!("latest_at", format!("{component_name} @ {query:?}"));

    let ((data_time, _row_id), unit) = store
        .latest_at_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        .filter_map(|chunk| {
            chunk
                .latest_at(query, component_name)
                .into_unit()
                .and_then(|chunk| chunk.index(&query.timeline()).map(|index| (index, chunk)))
        })
        .max_by_key(|(index, _chunk)| *index)?;

    // TODO: is there any value in returning a chunk here though?
    Some(unit.into_chunk())
}
