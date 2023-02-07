use arrow2::{
    array::{new_empty_array, Array, ListArray, UInt64Array},
    datatypes::DataType,
};
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use parking_lot::RwLock;

use re_log::{debug, trace};
use re_log_types::{
    msg_bundle::{wrap_in_listarray, ComponentBundle, MsgBundle},
    ComponentName, EntityPath, MsgId, TimeInt, TimePoint, TimeRange, Timeline,
};

use crate::{
    ArrayExt as _, ComponentBucket, ComponentTable, DataStore, DataStoreConfig, IndexBucket,
    IndexBucketIndices, IndexTable, PersistentComponentTable, PersistentIndexTable, RowIndex,
    RowIndexKind, TimeIndex,
};

// --- Data store ---

#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    // Batches
    #[error("Cannot insert more than 1 row at a time, got {0}")]
    MoreThanOneRow(usize),
    #[error("All components must have the same number of rows, got {0:?}")]
    MismatchedRows(Vec<(ComponentName, usize)>),

    // Clustering key
    #[error("The cluster component must be dense, got {0:?}")]
    SparseClusteringComponent(Box<dyn Array>),
    #[error(
        "The cluster component must be increasingly sorted and not contain \
                any duplicates, got {0:?}"
    )]
    InvalidClusteringComponent(Box<dyn Array>),

    // Instances
    #[error(
        "All components within a row must have the same number of instances as the \
            cluster component, got {cluster_comp}={cluster_comp_nb_instances} vs. \
                {key}={nb_instances}"
    )]
    MismatchedInstances {
        cluster_comp: ComponentName,
        cluster_comp_nb_instances: usize,
        key: ComponentName,
        nb_instances: usize,
    },

    // Misc
    #[error("Other error")]
    Other(#[from] anyhow::Error),
}

pub type WriteResult<T> = ::std::result::Result<T, WriteError>;

impl DataStore {
    /// Inserts a [`MsgBundle`]'s worth of components into the datastore.
    ///
    /// * All components across the bundle must share the same number of rows.
    /// * All components within a single row must share the same number of instances.
    ///
    /// If the bundle doesn't carry a payload for the cluster key, one will be auto-generated
    /// based on the length of the components in the payload, in the form of an array of
    /// monotonically increasing u64s going from `0` to `N-1`.
    pub fn insert(&mut self, msg: &MsgBundle) -> WriteResult<()> {
        // TODO(cmc): kind & insert_id need to somehow propagate through the span system.
        self.insert_id += 1;

        let MsgBundle {
            msg_id,
            entity_path: ent_path,
            time_point,
            components: bundles,
        } = msg;

        if bundles.is_empty() {
            return Ok(());
        }

        crate::profile_function!();

        let ent_path_hash = ent_path.hash();
        let nb_rows = bundles[0].nb_rows();

        // Effectively the same thing as having a non-unit length batch, except it's really not
        // worth more than an assertion since:
        // - A) `MsgBundle` should already guarantee this
        // - B) this limitation should be gone soon enough
        debug_assert!(
            msg.components
                .iter()
                .map(|bundle| bundle.name())
                .all_unique(),
            "cannot insert same component multiple times, this is equivalent to multiple rows",
        );
        // Batches cannot contain more than 1 row at the moment.
        if nb_rows != 1 {
            return Err(WriteError::MoreThanOneRow(nb_rows));
        }
        // Components must share the same number of rows.
        if !bundles.iter().all(|bundle| bundle.nb_rows() == nb_rows) {
            return Err(WriteError::MismatchedRows(
                bundles
                    .iter()
                    .map(|bundle| (bundle.name(), bundle.nb_rows()))
                    .collect(),
            ));
        }

        trace!(
            kind = "insert",
            id = self.insert_id,
            cluster_key = %self.cluster_key,
            timelines = ?time_point.iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            entity = %ent_path,
            components = ?bundles.iter().map(|bundle| bundle.name()).collect::<Vec<_>>(),
            nb_rows,
            "insertion started..."
        );

        let cluster_comp_pos = bundles
            .iter()
            .find_position(|bundle| bundle.name() == self.cluster_key)
            .map(|(pos, _)| pos);

        if time_point.is_timeless() {
            let mut row_indices = IntMap::default();

            // TODO(#589): support for batched row component insertions
            for row_nr in 0..nb_rows {
                self.insert_timeless_row(row_nr, cluster_comp_pos, bundles, &mut row_indices)?;
            }

            let index = self
                .timeless_indices
                .entry(ent_path_hash)
                .or_insert_with(|| PersistentIndexTable::new(self.cluster_key, ent_path.clone()));
            index.insert(&row_indices)?;
        } else {
            let mut row_indices = IntMap::default();

            // TODO(#589): support for batched row component insertions
            for row_nr in 0..nb_rows {
                self.insert_row(
                    time_point,
                    row_nr,
                    cluster_comp_pos,
                    bundles,
                    &mut row_indices,
                )?;
            }

            for (timeline, time) in time_point.iter() {
                let ent_path = ent_path.clone(); // shallow
                let index = self
                    .indices
                    .entry((*timeline, ent_path_hash))
                    .or_insert_with(|| IndexTable::new(self.cluster_key, *timeline, ent_path));
                index.insert(&self.config, *time, &row_indices)?;
            }
        }

        // This is valuable information, even for a timeless timepoint!
        self.messages.insert(*msg_id, time_point.clone());

        Ok(())
    }

    fn insert_timeless_row(
        &mut self,
        row_nr: usize,
        cluster_comp_pos: Option<usize>,
        components: &[ComponentBundle],
        row_indices: &mut IntMap<ComponentName, RowIndex>,
    ) -> WriteResult<()> {
        crate::profile_function!();

        let (cluster_row_idx, cluster_len) = self.get_or_create_cluster_component(
            row_nr,
            cluster_comp_pos,
            components,
            &TimePoint::default(),
        )?;

        // Always insert the cluster component.
        row_indices.insert(self.cluster_key, cluster_row_idx);

        if self.config.store_insert_ids {
            // Store the ID of the write request alongside the data.
            //
            // This is _not_ an actual `RowIndex`, there isn't even a component table associated
            // with insert IDs!
            // We're just abusing the fact that any value we push here as a `RowIndex` will end up
            // as-is in the index.
            row_indices.insert(
                Self::insert_id_key(),
                RowIndex::from_u63(RowIndexKind::Temporal, self.insert_id),
            );
        }

        for bundle in components
            .iter()
            .filter(|bundle| bundle.name() != self.cluster_key)
        {
            let (name, rows) = (bundle.name(), bundle.value_list());

            // Unwrapping a ListArray is somewhat costly, especially considering we're just
            // gonna rewrap it again in a minute... so we'd rather just slice it to a list of
            // one instead.
            //
            // let rows_single = rows.slice(row_nr, 1);
            //
            // Except it turns out that slicing is _extremely_ costly!
            // So use the fact that `rows` is always of unit-length for now.
            let rows_single = rows;

            let nb_instances = rows_single.offsets().lengths().next().unwrap();
            if nb_instances != cluster_len {
                return Err(WriteError::MismatchedInstances {
                    cluster_comp: self.cluster_key,
                    cluster_comp_nb_instances: cluster_len,
                    key: name,
                    nb_instances,
                });
            }

            let table = self
                .timeless_components
                .entry(bundle.name())
                .or_insert_with(|| {
                    PersistentComponentTable::new(
                        name,
                        ListArray::<i32>::get_child_type(rows_single.data_type()),
                    )
                });

            let row_idx = table.push(rows_single);
            row_indices.insert(name, row_idx);
        }

        Ok(())
    }

    fn insert_row(
        &mut self,
        time_point: &TimePoint,
        row_nr: usize,
        cluster_comp_pos: Option<usize>,
        components: &[ComponentBundle],
        row_indices: &mut IntMap<ComponentName, RowIndex>,
    ) -> WriteResult<()> {
        crate::profile_function!();

        let (cluster_row_idx, cluster_len) =
            self.get_or_create_cluster_component(row_nr, cluster_comp_pos, components, time_point)?;

        // Always insert the cluster component.
        row_indices.insert(self.cluster_key, cluster_row_idx);

        if self.config.store_insert_ids {
            // Store the ID of the write request alongside the data.
            //
            // This is _not_ an actual `RowIndex`, there isn't even a component table associated
            // with insert IDs!
            // We're just abusing the fact that any value we push here as a `RowIndex` will end up
            // as-is in the index.
            row_indices.insert(
                Self::insert_id_key(),
                RowIndex::from_u63(RowIndexKind::Temporal, self.insert_id),
            );
        }

        for bundle in components
            .iter()
            .filter(|bundle| bundle.name() != self.cluster_key)
        {
            let (name, rows) = (bundle.name(), bundle.value_list());

            // Unwrapping a ListArray is somewhat costly, especially considering we're just
            // gonna rewrap it again in a minute... so we'd rather just slice it to a list of
            // one instead.
            //
            // let rows_single = rows.slice(row_nr, 1);
            //
            // Except it turns out that slicing is _extremely_ costly!
            // So use the fact that `rows` is always of unit-length for now.
            let rows_single = rows;

            // TODO(#440): support for splats
            let nb_instances = rows_single.offsets().lengths().next().unwrap();
            if nb_instances != cluster_len {
                return Err(WriteError::MismatchedInstances {
                    cluster_comp: self.cluster_key,
                    cluster_comp_nb_instances: cluster_len,
                    key: name,
                    nb_instances,
                });
            }

            let table = self.components.entry(bundle.name()).or_insert_with(|| {
                ComponentTable::new(
                    name,
                    ListArray::<i32>::get_child_type(rows_single.data_type()),
                )
            });

            let row_idx = table.push(&self.config, time_point, rows_single);
            row_indices.insert(name, row_idx);
        }

        Ok(())
    }

    /// Tries to find the cluster component for the current row, or creates it if the caller hasn't
    /// specified any.
    ///
    /// When creating an auto-generated cluster component of a specific length for the first time,
    /// this will keep track of its assigned row index and re-use it later on as a mean of
    /// deduplication.
    fn get_or_create_cluster_component(
        &mut self,
        _row_nr: usize,
        cluster_comp_pos: Option<usize>,
        components: &[ComponentBundle],
        time_point: &TimePoint,
    ) -> WriteResult<(RowIndex, usize)> {
        crate::profile_function!();

        enum ClusterData<'a> {
            Cached(RowIndex),
            GenData(Box<dyn Array>),
            UserData(&'a ListArray<i32>),
        }

        let (cluster_len, cluster_data) = if let Some(cluster_comp_pos) = cluster_comp_pos {
            // We found a component with a name matching the cluster key's, let's make sure it's
            // valid (dense, sorted, no duplicates) and use that if so.

            let cluster_comp = &components[cluster_comp_pos];
            let data = cluster_comp.value_list().values(); // abusing the fact that nb_rows==1
            let len = data.len();

            // Clustering component must be dense.
            if !data.is_dense() {
                return Err(WriteError::SparseClusteringComponent(data.clone()));
            }
            // Clustering component must be sorted and not contain any duplicates.
            if !data.is_sorted_and_unique()? {
                return Err(WriteError::InvalidClusteringComponent(data.clone()));
            }

            (len, ClusterData::UserData(cluster_comp.value_list()))
        } else {
            // The caller has not specified any cluster component, and so we'll have to generate
            // one... unless we've already generated one of this exact length in the past,
            // in which case we can simply re-use that row index.

            // Use the length of any other component in the batch, they are guaranteed to all
            // share the same length at this point anyway.
            let len = components.first().map_or(0, |comp| {
                comp.value_list().offsets().lengths().next().unwrap()
            });

            if let Some(row_idx) = self.cluster_comp_cache.get(&len) {
                // Cache hit! Re-use that row index.
                (len, ClusterData::Cached(*row_idx))
            } else {
                // Cache miss! Craft a new u64 array from the ground up.
                let data = UInt64Array::from_vec((0..len as u64).collect_vec()).boxed();
                let data = wrap_in_listarray(data).to_boxed();
                (len, ClusterData::GenData(data))
            }
        };

        match cluster_data {
            ClusterData::Cached(row_idx) => Ok((row_idx, cluster_len)),
            ClusterData::GenData(data) => {
                // We had to generate a cluster component of the given length for the first time,
                // let's store it forever.

                let table = self
                    .timeless_components
                    .entry(self.cluster_key)
                    .or_insert_with(|| {
                        PersistentComponentTable::new(
                            self.cluster_key,
                            ListArray::<i32>::get_child_type(data.data_type()),
                        )
                    });
                let row_idx = table.push(&*data);

                self.cluster_comp_cache.insert(cluster_len, row_idx);

                Ok((row_idx, cluster_len))
            }
            ClusterData::UserData(data) => {
                // If we didn't hit the cache, then we have to insert this cluster component in
                // the right tables, just like any other component.

                let row_idx = if time_point.is_timeless() {
                    let table = self
                        .timeless_components
                        .entry(self.cluster_key)
                        .or_insert_with(|| {
                            PersistentComponentTable::new(
                                self.cluster_key,
                                ListArray::<i32>::get_child_type(data.data_type()),
                            )
                        });
                    table.push(data)
                } else {
                    let table = self.components.entry(self.cluster_key).or_insert_with(|| {
                        ComponentTable::new(
                            self.cluster_key,
                            ListArray::<i32>::get_child_type(data.data_type()),
                        )
                    });
                    table.push(&self.config, time_point, data)
                };

                Ok((row_idx, cluster_len))
            }
        }
    }

    pub fn clear_msg_metadata(&mut self, drop_msg_ids: &ahash::HashSet<MsgId>) {
        crate::profile_function!();

        self.messages
            .retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
    }
}

// --- Persistent Indices ---

impl PersistentIndexTable {
    pub fn new(cluster_key: ComponentName, ent_path: EntityPath) -> Self {
        Self {
            cluster_key,
            ent_path,
            indices: Default::default(),
            nb_rows: 0,
            all_components: Default::default(),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn insert(&mut self, row_indices: &IntMap<ComponentName, RowIndex>) -> anyhow::Result<()> {
        crate::profile_function!();

        // 2-way merge, step1: left-to-right
        //
        // push new row indices to their associated secondary index
        for (name, row_idx) in row_indices {
            let index = self
                .indices
                .entry(*name)
                .or_insert_with(|| vec![None; self.nb_rows as usize]);
            index.push(Some(*row_idx));
        }

        // 2-way merge, step2: right-to-left
        //
        // fill unimpacted secondary indices with null values
        for (name, index) in &mut self.indices {
            if !row_indices.contains_key(name) {
                index.push(None);
            }
        }

        self.nb_rows += 1;

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        // Insert components last, only if bucket-insert succeeded.
        self.all_components.extend(row_indices.keys());

        Ok(())
    }
}

// --- Indices ---

impl IndexTable {
    pub fn new(cluster_key: ComponentName, timeline: Timeline, ent_path: EntityPath) -> Self {
        Self {
            timeline,
            ent_path,
            buckets: [(i64::MIN.into(), IndexBucket::new(cluster_key, timeline))].into(),
            cluster_key,
            all_components: Default::default(),
        }
    }

    pub fn insert(
        &mut self,
        config: &DataStoreConfig,
        time: TimeInt,
        indices: &IntMap<ComponentName, RowIndex>,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

        // borrowck workaround
        let timeline = self.timeline;
        let ent_path = self.ent_path.clone(); // shallow

        let (_, bucket) = self.find_bucket_mut(time);

        let size = bucket.total_size_bytes();
        let size_overflow = bucket.total_size_bytes() > config.index_bucket_size_bytes;

        let len = bucket.total_rows();
        let len_overflow = len > config.index_bucket_nb_rows;

        if size_overflow || len_overflow {
            if let Some((min, second_half)) = bucket.split() {
                trace!(
                    kind = "insert",
                    timeline = %timeline.name(),
                    time = timeline.typ().format(time),
                    entity = %ent_path,
                    size_limit = config.component_bucket_size_bytes,
                    len_limit = config.component_bucket_nb_rows,
                    size, size_overflow,
                    len, len_overflow,
                    new_time_bound = timeline.typ().format(min),
                    "splitting off index bucket following overflow"
                );

                self.buckets.insert(min, second_half);
                return self.insert(config, time, indices);
            }

            // We couldn't split the bucket, either because it's already too small, or because it
            // contains a unique timepoint value that's repeated multiple times.
            //
            // * If the bucket is that small, then there really is no better thing to do than
            //   letting it grow some more by appending to it.
            //
            // * If the timepoint we're trying to insert is smaller or equal to the current upper
            //   bound of the bucket, then at this point we have no choice but to insert it here
            //   (by definition, it is impossible that any previous bucket in the chain covers a
            //   time range that includes this timepoint: buckets are non-overlapping!).
            //
            // * Otherwise, if the timepoint we're trying to insert is greater than the upper bound
            //   of the current bucket, then it means that there currently exist no bucket that
            //   covers a time range which includes this timepoint (if such a bucket existed, then
            //   we would have stumbled upon it before ever finding the current one!).
            //   This gives us an opportunity to create a new bucket that starts at the upper
            //   bound of the current one _excluded_ and that ranges all the way up to the timepoint
            //   that we're inserting.
            //   Not only is this a great opportunity to naturally split things up, it's actually
            //   mandatory to avoid a nasty edge case where one keeps inserting into a full,
            //   unsplittable bucket and indefinitely creates new single-entry buckets, leading
            //   to the worst-possible case of fragmentation.

            let (bucket_upper_bound, bucket_len) = {
                let guard = bucket.indices.read();
                (guard.times.last().copied(), guard.times.len())
            };

            if let Some(upper_bound) = bucket_upper_bound {
                if bucket_len > 2 && time.as_i64() > upper_bound {
                    let new_time_bound = upper_bound + 1;
                    debug!(
                        kind = "insert",
                        timeline = %timeline.name(),
                        time = timeline.typ().format(time),
                        entity = %ent_path,
                        size_limit = config.component_bucket_size_bytes,
                        len_limit = config.component_bucket_nb_rows,
                        size, size_overflow,
                        len, len_overflow,
                        new_time_bound = timeline.typ().format(new_time_bound.into()),
                        "creating brand new index bucket following overflow"
                    );
                    self.buckets.insert(
                        (new_time_bound).into(),
                        IndexBucket {
                            timeline,
                            indices: RwLock::new(IndexBucketIndices {
                                is_sorted: true,
                                time_range: TimeRange::new(time, time),
                                times: Default::default(),
                                indices: Default::default(),
                            }),
                            cluster_key: self.cluster_key,
                        },
                    );
                    return self.insert(config, time, indices);
                }
            }

            debug!(
                kind = "insert",
                timeline = %timeline.name(),
                time = timeline.typ().format(time),
                entity = %ent_path,
                size_limit = config.component_bucket_size_bytes,
                len_limit = config.component_bucket_nb_rows,
                size, size_overflow,
                len, len_overflow,
                "couldn't split index bucket, proceeding to ignore limits"
            );
        }

        trace!(
            kind = "insert",
            timeline = %timeline.name(),
            time = timeline.typ().format(time),
            entity = %ent_path,
            components = ?indices.iter().collect::<Vec<_>>(),
            "inserted into index table"
        );

        bucket.insert(time, indices)?;

        // Insert components last, only if bucket-insert succeeded.
        self.all_components.extend(indices.keys());

        Ok(())
    }
}

impl IndexBucket {
    pub fn new(cluster_key: ComponentName, timeline: Timeline) -> Self {
        Self {
            timeline,
            indices: RwLock::new(IndexBucketIndices::default()),
            cluster_key,
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn insert(
        &mut self,
        time: TimeInt,
        row_indices: &IntMap<ComponentName, RowIndex>,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

        let mut guard = self.indices.write();
        let IndexBucketIndices {
            is_sorted,
            time_range,
            times,
            indices,
        } = &mut *guard;

        // append time to primary index and update time range approriately
        times.push(time.as_i64());
        *time_range = TimeRange::new(time_range.min.min(time), time_range.max.max(time));

        // append components to secondary indices (2-way merge)

        // 2-way merge, step1: left-to-right
        //
        // push new row indices to their associated secondary index
        for (name, row_idx) in row_indices {
            let index = indices
                .entry(*name)
                .or_insert_with(|| vec![None; times.len().saturating_sub(1)]);
            index.push(Some(*row_idx));
        }

        // 2-way merge, step2: right-to-left
        //
        // fill unimpacted secondary indices with null values
        for (name, index) in &mut *indices {
            if !row_indices.contains_key(name) {
                index.push(None);
            }
        }

        // TODO(#433): re_datastore: properly handle already sorted data during insertion
        *is_sorted = false;

        #[cfg(debug_assertions)]
        {
            drop(guard); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
        }

        Ok(())
    }

    /// Splits the bucket into two, potentially uneven parts.
    ///
    /// On success..:
    /// - the first part is split in place (i.e. modifies `self`),
    /// - the second part is returned as a new bucket,
    /// - and the minimal bound of that new bucket is returned as a `TimeInt`, for indexing.
    ///
    /// Returns `None` on failure, i.e. if the bucket cannot be split any further, which can
    /// happen either because the bucket is too small to begin with, or because it only contains
    /// a single timepoint.
    ///
    /// # Unsplittable buckets
    ///
    /// The datastore and query path operate under the general assumption that _all of the
    /// index data_ for a given timepoint will reside in _one and only one_ bucket.
    /// This function makes sure to uphold that restriction, which sometimes means splitting the
    /// bucket into two uneven parts, or even not splitting it at all.
    ///
    /// Here's an example of an index table configured to have a maximum of 2 rows per bucket: one
    /// can see that the 1st and 2nd buckets exceed this maximum in order to uphold the restriction
    /// described above:
    /// ```text
    /// IndexTable {
    ///     timeline: frame_nr
    ///     entity: this/that
    ///     size: 3 buckets for a total of 256 B across 8 total rows
    ///     buckets: [
    ///         IndexBucket {
    ///             index time bound: >= #0
    ///             size: 96 B across 3 rows
    ///                 - frame_nr: from #41 to #41 (all inclusive)
    ///             data (sorted=true):
    ///             +----------+---------------+--------------+--------------------+
    ///             | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance_key |
    ///             +----------+---------------+--------------+--------------------+
    ///             | 41       |               |              | 1                  |
    ///             | 41       | 1             |              | 2                  |
    ///             | 41       |               | 3            | 2                  |
    ///             +----------+---------------+--------------+--------------------+
    ///
    ///         }
    ///         IndexBucket {
    ///             index time bound: >= #42
    ///             size: 96 B across 3 rows
    ///                 - frame_nr: from #42 to #42 (all inclusive)
    ///             data (sorted=true):
    ///             +----------+--------------+--------------------+--------------------+
    ///             | frame_nr | rerun.rect2d | rerun.instance_key | rerun.point2d |
    ///             +----------+--------------+--------------------+-------------------+
    ///             | 42       | 1            | 2                  |                   |
    ///             | 42       |              | 4                  |                   |
    ///             | 42       |              | 2                  | 2                 |
    ///             +----------+--------------+--------------------+-------------------+
    ///
    ///         }
    ///         IndexBucket {
    ///             index time bound: >= #43
    ///             size: 64 B across 2 rows
    ///                 - frame_nr: from #43 to #44 (all inclusive)
    ///             data (sorted=true):
    ///             +----------+--------------+---------------+--------------------+
    ///             | frame_nr | rerun.rect2d | rerun.point2d | rerun.instance_key |
    ///             +----------+--------------+---------------+--------------------+
    ///             | 43       | 4            |               | 2                  |
    ///             | 44       |              | 3             | 2                  |
    ///             +----------+--------------+---------------+--------------------+
    ///
    ///         }
    ///     ]
    /// }
    /// ```
    pub fn split(&self) -> Option<(TimeInt, Self)> {
        let Self {
            timeline, indices, ..
        } = self;

        let mut indices = indices.write();
        indices.sort();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: time_range1,
            times: times1,
            indices: indices1,
        } = &mut *indices;

        if times1.len() < 2 {
            return None; // early exit: can't split the unsplittable
        }

        if times1.first() == times1.last() {
            // The entire bucket contains only one timepoint, thus it's impossible to find
            // a split index to begin with.
            return None;
        }

        crate::profile_function!();

        let timeline = *timeline;
        // Used down the line to assert that we've left everything in a sane state.
        let _total_rows = times1.len();

        let (min2, bucket2) = {
            let split_idx = find_split_index(times1).expect("must be splittable at this point");

            // this updates `time_range1` in-place!
            let time_range2 = split_time_range_off(split_idx, times1, time_range1);

            // this updates `times1` in-place!
            let times2 = times1.split_off(split_idx);

            // this updates `indices1` in-place!
            let indices2: IntMap<_, _> = indices1
                .iter_mut()
                .map(|(name, index1)| {
                    // this updates `index1` in-place!
                    let index2 = index1.split_off(split_idx);
                    (*name, index2)
                })
                .collect();
            (
                time_range2.min,
                Self {
                    timeline,
                    indices: RwLock::new(IndexBucketIndices {
                        is_sorted: true,
                        time_range: time_range2,
                        times: times2,
                        indices: indices2,
                    }),
                    cluster_key: self.cluster_key,
                },
            )
        };

        // sanity checks
        #[cfg(debug_assertions)]
        {
            drop(indices); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
            bucket2.sanity_check().unwrap();

            let total_rows1 = self.total_rows() as i64;
            let total_rows2 = bucket2.total_rows() as i64;
            debug_assert!(
                _total_rows as i64 == total_rows1 + total_rows2,
                "expected both buckets to sum up to the length of the original bucket: \
                    got bucket={} vs. bucket1+bucket2={}",
                _total_rows,
                total_rows1 + total_rows2,
            );
            debug_assert_eq!(_total_rows as i64, total_rows1 + total_rows2);
        }

        Some((min2, bucket2))
    }
}

/// Finds an optimal split point for the given time index, or `None` if all entries in the index
/// are identical, making it unsplittable.
///
/// The returned index is _exclusive_: `[0, split_idx)` + `[split_idx; len)`.
///
/// # Panics
///
/// This function expects `times` to be sorted!
/// In debug builds, it will panic if that's not the case.
fn find_split_index(times: &TimeIndex) -> Option<usize> {
    debug_assert!(
        times.windows(2).all(|t| t[0] <= t[1]),
        "time index must be sorted before splitting!"
    );

    if times.first() == times.last() {
        return None; // early exit: unsplittable
    }

    crate::profile_function!();

    // This can never be lesser than 1 as we never split buckets smaller than 2 entries.
    let halfway_idx = times.len() / 2;
    let target = times[halfway_idx];

    // Are we about to split in the middle of a continuous run? Hop backwards to figure it out.
    let split_idx1 = Some(times[..halfway_idx].partition_point(|&t| t < target)).filter(|&i| i > 0);

    // Are we about to split in the middle of a continuous run? Hop forwards to figure it out.
    let split_idx2 = Some(times[halfway_idx..].partition_point(|&t| t <= target))
        .map(|t| t + halfway_idx) // we skipped that many entries!
        .filter(|&t| t < times.len());

    // Are we in the middle of a backwards continuous run? a forwards continuous run? both?
    match (split_idx1, split_idx2) {
        // Unsplittable, which cannot happen as we already early-exit earlier.
        #[cfg(not(debug_assertions))]
        (None, None) => None,
        #[cfg(debug_assertions)]
        (None, None) => unreachable!(),

        // Backwards run, let's use the first split index.
        (Some(split_idx1), None) => Some(split_idx1),

        // Forwards run, let's use the second split index.
        (None, Some(split_idx2)) => Some(split_idx2),

        // The run goes both backwards and forwards from the half point: use the split index
        // that's the closest to halfway.
        (Some(split_idx1), Some(split_idx2)) => {
            if halfway_idx.abs_diff(split_idx1) < halfway_idx.abs_diff(split_idx2) {
                split_idx1
            } else {
                split_idx2
            }
            .into()
        }
    }
}

#[test]
fn test_find_split_index() {
    let test_cases = [
        (vec![1, 1], None),
        //
        (vec![1, 1, 1], None),
        (vec![1, 1, 2], Some(2)),
        (vec![0, 1, 1], Some(1)),
        //
        (vec![1, 1, 1, 1], None),
        (vec![1, 1, 1, 2], Some(3)),
        (vec![0, 1, 1, 1], Some(1)),
        //
        (vec![1, 1, 1, 1, 1], None),
        (vec![1, 1, 1, 1, 2], Some(4)),
        (vec![0, 1, 1, 1, 1], Some(1)),
        (vec![0, 1, 1, 1, 2], Some(1)), // first one wins when equal distances
        (vec![0, 1, 1, 2, 2], Some(3)), // second one is closer
        (vec![0, 0, 1, 2, 2], Some(2)), // first one wins when equal distances
        (vec![0, 0, 2, 2, 2], Some(2)), // second one is closer
        (vec![0, 0, 0, 2, 2], Some(3)), // first one is closer
    ];

    for (times, expected) in test_cases {
        let got = find_split_index(&times);
        assert_eq!(expected, got);
    }
}

/// Given a time index and a desired split index, splits off the given time range in place,
/// and returns a new time range corresponding to the second part.
///
/// The split index is exclusive: everything up to `split_idx` (excluded) will end up in the
/// first split.
///
/// The two resulting time range halves are guaranteed to never overlap.
fn split_time_range_off(
    split_idx: usize,
    times1: &TimeIndex,
    time_range1: &mut TimeRange,
) -> TimeRange {
    let time_range2 = TimeRange::new(times1[split_idx].into(), time_range1.max);

    // This can never fail (underflow or OOB) because we never split buckets smaller than 2
    // entries.
    time_range1.max = times1[split_idx - 1].into();

    debug_assert!(
        time_range1.max.as_i64() < time_range2.min.as_i64(),
        "split resulted in overlapping time ranges: {} <-> {}\n{:#?}",
        time_range1.max.as_i64(),
        time_range2.min.as_i64(),
        (&time_range1, &time_range2),
    );

    time_range2
}

// --- Persistent Components ---

impl PersistentComponentTable {
    /// Creates a new timeless component table for the specified component `datatype`.
    ///
    /// `datatype` must be the type of the component itself, devoid of any wrapping layers
    /// (i.e. _not_ a `ListArray<...>`!).
    fn new(name: ComponentName, datatype: &DataType) -> Self {
        // TODO(cmc): think about this when implementing deletion.
        let chunks = vec![wrap_in_listarray(new_empty_array(datatype.clone())).to_boxed()];
        let total_rows = chunks.iter().map(|values| values.len() as u64).sum();
        let total_size_bytes = chunks
            .iter()
            .map(|values| arrow2::compute::aggregate::estimated_bytes_size(&**values) as u64)
            .sum();

        Self {
            name,
            datatype: datatype.clone(),
            chunks,
            total_rows,
            total_size_bytes,
        }
    }

    /// Pushes `rows_single` to the end of the bucket, returning the _global_ `RowIndex` of the
    /// freshly added row.
    ///
    /// `rows_single` must be a unit-length list of arrays of structs,
    /// i.e. `ListArray<StructArray>`:
    /// - the list layer corresponds to the different rows (always unit-length for now),
    /// - the array layer corresponds to the different instances within that single row,
    /// - and finally the struct layer holds the components themselves.
    /// E.g.:
    /// ```text
    /// [[{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}]]
    /// ```
    //
    // TODO(#589): support for batched row component insertions
    pub fn push(&mut self, rows_single: &dyn Array) -> RowIndex {
        crate::profile_function!();

        debug_assert!(
            ListArray::<i32>::get_child_type(rows_single.data_type()) == &self.datatype,
            "trying to insert data of the wrong datatype in a component table, \
                expected {:?}, got {:?}",
            &self.datatype,
            ListArray::<i32>::get_child_type(rows_single.data_type()),
        );
        debug_assert!(
            rows_single.len() == 1,
            "batched row component insertions are not supported yet"
        );

        self.total_rows += 1;
        // Warning: this is surprisingly costly!
        self.total_size_bytes +=
            arrow2::compute::aggregate::estimated_bytes_size(rows_single) as u64;

        // TODO(#589): support for non-unit-length chunks
        self.chunks.push(rows_single.to_boxed()); // shallow

        RowIndex::from_u63(RowIndexKind::Timeless, self.chunks.len() as u64 - 1)
    }
}

// --- Components ---

impl ComponentTable {
    /// Creates a new component table for the specified component `datatype`.
    ///
    /// `datatype` must be the type of the component itself, devoid of any wrapping layers
    /// (i.e. _not_ a `ListArray<...>`!).
    fn new(name: ComponentName, datatype: &DataType) -> Self {
        ComponentTable {
            name,
            datatype: datatype.clone(),
            buckets: [ComponentBucket::new(name, datatype, 0u64)].into(),
        }
    }

    /// Finds the appropriate bucket in this component table and pushes `rows_single` at the
    /// end of it, returning the _global_ `RowIndex` for this new row.
    ///
    /// `rows_single` must be a unit-length list of arrays of structs,
    /// i.e. `ListArray<StructArray>`:
    /// - the list layer corresponds to the different rows (always unit-length for now),
    /// - the array layer corresponds to the different instances within that single row,
    /// - and finally the struct layer holds the components themselves.
    /// E.g.:
    /// ```text
    /// [[{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}]]
    /// ```
    //
    // TODO(#589): support for batched row component insertions
    pub fn push(
        &mut self,
        config: &DataStoreConfig,
        time_point: &TimePoint,
        rows_single: &dyn Array,
    ) -> RowIndex {
        crate::profile_function!();

        debug_assert!(
            ListArray::<i32>::get_child_type(rows_single.data_type()) == &self.datatype,
            "trying to insert data of the wrong datatype in a component table, \
                expected {:?}, got {:?}",
            &self.datatype,
            ListArray::<i32>::get_child_type(rows_single.data_type()),
        );
        debug_assert!(
            rows_single.len() == 1,
            "batched row component insertions are not supported yet"
        );

        // All component tables spawn with an initial bucket at row offset 0, thus this cannot
        // fail.
        let active_bucket = self.buckets.back_mut().unwrap();

        let size = active_bucket.total_size_bytes();
        let size_overflow = active_bucket.total_size_bytes() > config.component_bucket_size_bytes;

        let len = active_bucket.total_rows();
        let len_overflow = len > config.component_bucket_nb_rows;

        if size_overflow || len_overflow {
            trace!(
                kind = "insert",
                component = self.name.as_str(),
                size_limit = config.component_bucket_size_bytes,
                len_limit = config.component_bucket_nb_rows,
                size,
                size_overflow,
                len,
                len_overflow,
                "allocating new component bucket, previous one overflowed"
            );

            // Archive currently active bucket.
            active_bucket.archive();

            let row_offset = active_bucket.row_offset + len;
            self.buckets
                .push_back(ComponentBucket::new(self.name, &self.datatype, row_offset));
        }

        // Two possible cases:
        // - If the table has not just underwent an overflow, then this is panic-safe for the
        //   same reason as above: all component tables spawn with an initial bucket at row
        //   offset 0, thus this cannot fail.
        // - If the table has just overflowed, then we've just pushed a bucket to the dequeue.
        let active_bucket = self.buckets.back_mut().unwrap();
        let row_idx = RowIndex::from_u63(
            RowIndexKind::Temporal,
            active_bucket.push(time_point, rows_single) + active_bucket.row_offset,
        );

        trace!(
            kind = "insert",
            timelines = ?time_point.into_iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            component = self.name.as_str(),
            %row_idx,
            "pushed into component table"
        );

        row_idx
    }
}

impl ComponentBucket {
    /// Creates a new component bucket for the specified component `datatype`.
    ///
    /// `datatype` must be the type of the component itself, devoid of any wrapping layers
    /// (i.e. _not_ a `ListArray<...>`!).
    pub fn new(name: ComponentName, datatype: &DataType, row_offset: u64) -> Self {
        // If this is the first bucket of this table, we need to insert an empty list at
        // row index #0!
        let chunks = if row_offset == 0 {
            vec![wrap_in_listarray(new_empty_array(datatype.clone())).to_boxed()]
        } else {
            vec![]
        };

        let total_rows = chunks.iter().map(|values| values.len() as u64).sum();
        let total_size_bytes = chunks
            .iter()
            .map(|values| arrow2::compute::aggregate::estimated_bytes_size(&**values) as u64)
            .sum();

        Self {
            name,
            row_offset,
            archived: false,
            time_ranges: Default::default(),
            chunks,
            total_rows,
            total_size_bytes,
        }
    }

    /// Pushes `rows_single` to the end of the bucket, returning the _local_ index of the
    /// freshly added row.
    ///
    /// `rows_single` must be a unit-length list of arrays of structs,
    /// i.e. `ListArray<StructArray>`:
    /// - the list layer corresponds to the different rows (always unit-length for now),
    /// - the array layer corresponds to the different instances within that single row,
    /// - and finally the struct layer holds the components themselves.
    /// E.g.:
    /// ```text
    /// [[{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}]]
    /// ```
    pub fn push(&mut self, time_point: &TimePoint, rows_single: &dyn Array) -> u64 {
        crate::profile_function!();

        debug_assert!(
            rows_single.len() == 1,
            "batched row component insertions are not supported yet"
        );

        // Keep track of all affected time ranges, for garbage collection purposes.
        for (timeline, &time) in time_point {
            self.time_ranges
                .entry(*timeline)
                .and_modify(|range| {
                    *range = TimeRange::new(range.min.min(time), range.max.max(time));
                })
                .or_insert_with(|| TimeRange::new(time, time));
        }

        self.total_rows += 1;
        // Warning: this is surprisingly costly!
        self.total_size_bytes +=
            arrow2::compute::aggregate::estimated_bytes_size(rows_single) as u64;

        // TODO(#589): support for non-unit-length chunks
        self.chunks.push(rows_single.to_boxed()); // shallow

        self.chunks.len() as u64 - 1
    }

    /// Archives the bucket as a new one is about to take its place.
    ///
    /// This is a good opportunity to run compaction and other maintenance related tasks.
    pub fn archive(&mut self) {
        crate::profile_function!();

        debug_assert!(
            !self.archived,
            "achiving an already archived bucket, something is likely wrong"
        );

        // Chunk compaction
        // Compacts the bucket by concatenating all chunks of data into a single one.
        {
            use arrow2::compute::concatenate::concatenate;

            let chunks = self.chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            // Only two reasons this can ever fail:
            //
            // * `chunks` is empty:
            // This can never happen, buckets always spawn with an initial chunk.
            //
            // * the various chunks contain data with different datatypes:
            // This can never happen as that would first panic during insertion.
            let values = concatenate(&chunks).unwrap();

            // Recompute the size as we've just discarded a bunch of list headers.
            self.total_size_bytes =
                arrow2::compute::aggregate::estimated_bytes_size(&*values) as u64;

            self.chunks = vec![values];
        }

        self.archived = true;
    }
}
