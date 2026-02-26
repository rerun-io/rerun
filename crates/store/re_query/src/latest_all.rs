use std::sync::Arc;

use nohash_hasher::IntMap;
use re_chunk::{
    Chunk, ChunkId, ComponentIdentifier, EntityPath, LatestAtQuery, RangeQuery, RowId,
    UnitChunkShared,
};
use re_chunk_store::TimeInt;
use re_log_types::AbsoluteTimeRange;

use crate::{LatestAtResults, QueryCache};

pub type LatestAllQuery = LatestAtQuery;

/// See [`QueryCache::latest_all`]
#[derive(Debug)]
pub struct LatestAllResults {
    /// The entity we queried.
    pub entity_path: EntityPath,

    /// The query that yielded these results.
    pub query: LatestAllQuery,

    /// The relevant *virtual* chunks that were found for this query.
    ///
    /// Until these chunks have been fetched and inserted into the appropriate [`re_chunk_store::ChunkStore`], the
    /// results of this query cannot accurately be computed.
    ///
    /// Note, these are NOT necessarily _root_ chunks.
    /// Use [`re_chunk_store::ChunkStore::find_root_chunks`] to get those.
    //
    // TODO(cmc): Once lineage tracking is in place, make sure that this only reports missing
    // chunks using their root-level IDs, so downstream consumers don't have to redundantly build
    // their own tracking. And document it so.
    pub missing_virtual: Vec<ChunkId>,

    /// Results for each individual component.
    ///
    /// If the component was not found, it will not appear in this list.
    pub components: IntMap<ComponentIdentifier, LatestAllComponentResults>,
}

impl LatestAllResults {
    /// Total number of hits across all components.
    ///
    /// If we have two components, and one has a single hit and another has four hits, this will return `5`.
    pub fn num_rows_total(&self) -> usize {
        self.components
            .values()
            .map(|component_results| component_results.num_rows())
            .sum()
    }

    /// If we have exactly one hit per component, return this as a [`LatestAtResults`].
    ///
    /// Returns `None` if any component has zero or more than one hit.
    pub fn try_as_latest_at(&self) -> Option<LatestAtResults> {
        let mut results = LatestAtResults {
            entity_path: self.entity_path.clone(),
            query: self.query.clone(),
            missing_virtual: self.missing_virtual.clone(),
            min_index: (TimeInt::MAX, RowId::MAX),
            max_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        };

        for (&component, component_results) in &self.components {
            if component_results.num_rows() != 1 {
                return None;
            }

            // We have exactly one row, so there should be exactly one chunk with one row
            let chunk = component_results.chunks.first()?;
            let unit = chunk.to_unit()?;

            let index = unit.index(&self.query.timeline())?;
            results.min_index = results.min_index.min(index);
            results.max_index = results.max_index.max(index);
            results.components.insert(component, unit);
        }

        Some(results)
    }

    /// Converts this into a [`LatestAtResults`] by extracting the latest row
    /// (highest `RowId`) for each component.
    ///
    /// Components with no rows are omitted from the result.
    pub fn into_latest_at(self) -> LatestAtResults {
        let mut results = LatestAtResults {
            entity_path: self.entity_path.clone(),
            query: self.query.clone(),
            missing_virtual: self.missing_virtual.clone(),
            min_index: (TimeInt::MAX, RowId::MAX),
            max_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        };

        for (component, component_results) in self.components {
            if let Some(unit) = component_results.latest_row()
                && let Some(row_id) = unit.row_id()
            {
                let index = (component_results.time, row_id);
                results.min_index = results.min_index.min(index);
                results.max_index = results.max_index.max(index);
                results.components.insert(component, unit);
            }
        }

        results
    }
}

/// See [`QueryCache::latest_all`]
#[derive(Clone, Debug)]
pub struct LatestAllComponentResults {
    time: TimeInt,

    /// We may have zero chunks, but each chunk is non-empty.
    chunks: Vec<Arc<Chunk>>,
}

impl LatestAllComponentResults {
    pub fn new(time: TimeInt, chunks: Vec<Arc<Chunk>>) -> Self {
        // TODO(emilk): consider converting to `Vec<UnitChunkShared>` right away
        Self { time, chunks }
    }

    pub fn from_unit(time: TimeInt, unit: UnitChunkShared) -> Self {
        Self {
            time,
            chunks: vec![unit.into_chunk()],
        }
    }

    /// At what time all the hits are at
    pub fn time(&self) -> TimeInt {
        self.time
    }

    /// Number of hits. Guaranteed to be non-zero.
    pub fn num_rows(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.num_rows()).sum()
    }

    /// Maximum number of instances found in any of the hits.
    ///
    /// This is e.g. number of points in a point cloud.
    pub fn max_num_instances(&self, component: ComponentIdentifier) -> u64 {
        self.iter_units()
            .map(|unit| unit.num_instances(component))
            .max()
            .unwrap_or(0)
    }

    /// Iterate over all hits, sorted by [`RowId`] order.
    pub fn iter_units(&self) -> impl Iterator<Item = UnitChunkShared> {
        let mut units: Vec<UnitChunkShared> = self
            .chunks
            .iter()
            .flat_map(|chunk| {
                (0..chunk.num_rows()).map(|row_index| chunk.row_sliced_unit_shallow(row_index))
            })
            .collect();
        // We need to sort, because the chunks could theoretically be interleaved.
        units.sort_by_key(|unit| unit.row_id());
        units.into_iter()
    }

    /// Returns an iterator over all component batches (one `Vec<C>` per row).
    ///
    /// The `component` parameter specifies which component to extract from each chunk.
    /// Rows where deserialization fails are skipped.
    pub fn iter_component_batches<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = Vec<C>> + '_ {
        self.chunks.iter().flat_map(move |chunk| {
            (0..chunk.num_rows())
                .filter_map(move |row_index| chunk.component_batch::<C>(component, row_index)?.ok())
        })
    }

    /// Returns the row with the highest [`RowId`].
    // TODO(emilk): have this return a non-Option, since we always have at least one hit
    pub fn latest_row(&self) -> Option<UnitChunkShared> {
        let mut best: Option<(UnitChunkShared, RowId)> = None;

        for chunk in &self.chunks {
            for (row_index, row_id) in chunk.row_ids().enumerate() {
                let dominated = best
                    .as_ref()
                    .is_some_and(|(_, best_row_id)| row_id <= *best_row_id);
                if dominated {
                    continue;
                }

                let unit = chunk.row_sliced_unit_shallow(row_index);
                let row_id = unit.row_id()?;
                best = Some((unit, row_id));
            }
        }

        re_log::debug_assert!(
            best.is_some(),
            "Each LatestAll should have at least one hit"
        );

        Some(best?.0)
    }

    /// If [`Self::num_rows`] == 1, return as [`UnitChunkShared`].
    pub fn try_as_unit(&self) -> Option<UnitChunkShared> {
        if self.num_rows() == 1 {
            self.latest_row()
        } else {
            None
        }
    }
}

impl QueryCache {
    /// Like [`Self::latest_at`], but may return multiple rows for each component,
    /// if those rows were all logged with the exact same [`TimeInt`].
    ///
    /// For instance: if you log many transforms to the same entity on the same timestep,
    /// only one of them will show up in a latest-at query, but all in a latest-all.
    ///
    /// In case of static data, only ONE value will ever be returned.
    /// This is because the store only ever keeps the last static value of everything.
    /// This, in turn, is because some users log e.g. a video stream
    /// as one static image after the other.
    pub fn latest_all(
        &self,
        query: &LatestAllQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> LatestAllResults {
        re_tracing::profile_function!();

        let LatestAtResults {
            components,
            missing_virtual,
            ..
        } = self.latest_at(query, entity_path, components);

        let mut latest_all_results = LatestAllResults {
            entity_path: entity_path.clone(),
            query: query.clone(),
            missing_virtual,
            components: Default::default(),
        };

        for (component, latest_unit) in components {
            if let Some((time, _row_id)) = latest_unit.index(&query.timeline()) {
                if time.is_static() {
                    latest_all_results.components.insert(
                        component,
                        LatestAllComponentResults::from_unit(time, latest_unit),
                    );
                } else {
                    let range_query =
                        RangeQuery::new(query.timeline(), AbsoluteTimeRange::new(time, time));
                    let mut component_range_result =
                        self.range(&range_query, entity_path, std::iter::once(component));

                    latest_all_results
                        .missing_virtual
                        .append(&mut component_range_result.missing_virtual);

                    if let Some(chunks) = component_range_result.components.remove(&component) {
                        latest_all_results.components.insert(
                            component,
                            LatestAllComponentResults::new(
                                time,
                                chunks.into_iter().map(Arc::new).collect(),
                            ),
                        );
                    }
                }
            }
        }

        latest_all_results.missing_virtual.sort();
        latest_all_results.missing_virtual.dedup();

        latest_all_results
    }
}
