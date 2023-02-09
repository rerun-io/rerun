use arrow2::{
    array::{new_empty_array, Array, BooleanArray, ListArray, UInt64Array, Utf8Array},
    bitmap::Bitmap,
    compute::concatenate::concatenate,
    offset::Offsets,
};
use nohash_hasher::IntMap;
use polars_core::{functions::diag_concat_df, prelude::*};
use re_log_types::ComponentName;

use crate::{
    store::SecondaryIndex, ArrayExt, DataStore, DataStoreConfig, IndexBucket, IndexBucketIndices,
    PersistentIndexTable, RowIndex,
};

// ---

impl DataStore {
    /// Dumps the entire datastore as a flat, denormalized dataframe.
    ///
    /// This cannot fail: it always tries to yield as much valuable information as it can, even in
    /// the face of errors.
    pub fn to_dataframe(&self) -> DataFrame {
        crate::profile_function!();

        const IS_TIMELESS_COL: &str = "_is_timeless";

        let timeless_dfs = self.timeless_indices.values().map(|index| {
            let ent_path = index.ent_path.clone();

            let mut df = index.to_dataframe(self, &self.config);
            let num_rows = df.get_columns()[0].len();

            // Add a column where every row is a boolean true (timeless)
            let timeless = {
                let timeless = BooleanArray::from(vec![Some(true); num_rows]).boxed();
                new_infallible_series(IS_TIMELESS_COL, timeless.as_ref(), num_rows)
            };
            let df = df.with_column(timeless).unwrap(); // cannot fail

            (ent_path, df.clone())
        });

        let temporal_dfs = self.indices.values().map(|index| {
            let dfs: Vec<_> = index
                .buckets
                .values()
                .map(|bucket| (index.ent_path.clone(), bucket))
                .map(|(ent_path, bucket)| {
                    let mut df = bucket.to_dataframe(self, &self.config);
                    let num_rows = df.get_columns()[0].len();

                    // Add a column where every row is the entity path.
                    let entities = {
                        let ent_path = ent_path.to_string();
                        let ent_path = Some(ent_path.as_str());
                        let entities = Utf8Array::<i32>::from(vec![ent_path; num_rows]).boxed();
                        new_infallible_series("entity", entities.as_ref(), num_rows)
                    };
                    let df = df.with_column(entities).unwrap(); // cannot fail

                    df.clone()
                })
                .collect();

            // Concatenate all buckets of the index together.
            //
            // This has to be done diagonally since each bucket can and will have different
            // numbers of columns (== components) and rows.
            let df = diag_concat_df(dfs.as_slice())
                // TODO(cmc): is there any way this can fail in this case?
                .unwrap();

            (index.ent_path.clone(), df)
        });

        let dfs: Vec<_> = timeless_dfs
            .chain(temporal_dfs)
            .map(|(ent_path, mut df)| {
                let num_rows = df.get_columns()[0].len();
                // Add a column where every row is the entity path.
                let entities = {
                    let ent_path = ent_path.to_string();
                    let ent_path = Some(ent_path.as_str());
                    let entities = Utf8Array::<i32>::from(vec![ent_path; num_rows]).boxed();
                    new_infallible_series("entity", entities.as_ref(), num_rows)
                };
                df.with_column(entities).unwrap().clone() // cannot fail
            })
            .collect();

        // Concatenate all indices together.
        //
        // This has to be done diagonally since these indices refer to different entities with
        // potentially wildly different sets of components and lengths.
        let df = diag_concat_df(dfs.as_slice())
            // TODO(cmc): is there any way this can fail in this case?
            .unwrap();

        let df = sort_df_columns(&df, self.config.store_insert_ids);

        let has_insert_ids = df.column(DataStore::insert_id_key().as_str()).is_ok();
        let has_timeless = df.column(IS_TIMELESS_COL).is_ok();

        let (sort_cols, sort_orders): (Vec<_>, Vec<_>) = [
            has_timeless.then_some((IS_TIMELESS_COL, true)),
            has_insert_ids.then_some((DataStore::insert_id_key().as_str(), false)),
        ]
        .into_iter()
        .flatten()
        .unzip();

        let df = if !sort_cols.is_empty() {
            df.sort(sort_cols, sort_orders).unwrap()
        } else {
            df
        };

        if has_timeless {
            df.drop(IS_TIMELESS_COL).unwrap()
        } else {
            df
        }
    }
}

impl PersistentIndexTable {
    /// Dumps the entire table as a flat, denormalized dataframe.
    ///
    /// This cannot fail: it always tries to yield as much valuable information as it can, even in
    /// the face of errors.
    pub fn to_dataframe(&self, store: &DataStore, config: &DataStoreConfig) -> DataFrame {
        crate::profile_function!();

        let Self {
            ent_path: _,
            cluster_key: _,
            num_rows,
            indices,
            all_components: _,
        } = self;

        let insert_ids = config
            .store_insert_ids
            .then(|| insert_ids_as_series(*num_rows as usize, indices))
            .flatten();

        let comp_series =
        // One column for insert IDs, if they are available.
        std::iter::once(insert_ids)
            .flatten() // filter options
            // One column for each component index.
            .chain(indices.iter().filter_map(|(component, comp_row_nrs)| {
            let datatype = find_component_datatype(store, component)?;
                component_as_series(store, *num_rows as usize, datatype, *component, comp_row_nrs).into()
            }));

        DataFrame::new(comp_series.collect::<Vec<_>>())
            // This cannot fail at this point, all series are guaranteed to have data and be of
            // same length.
            .unwrap()
    }
}

impl IndexBucket {
    /// Dumps the entire bucket as a flat, denormalized dataframe.
    ///
    /// This cannot fail: it always tries to yield as much valuable information as it can, even in
    /// the face of errors.
    pub fn to_dataframe(&self, store: &DataStore, config: &DataStoreConfig) -> DataFrame {
        crate::profile_function!();

        let (_, times) = self.times();
        let num_rows = times.len();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times: _,
            indices,
        } = &*self.indices.read();

        let insert_ids = config
            .store_insert_ids
            .then(|| insert_ids_as_series(num_rows, indices))
            .flatten();

        // Need to create one `Series` for the time index and one for each component index.
        let comp_series = [
            // One column for insert IDs, if they are available.
            insert_ids,
            // One column for the time index.
            Some(new_infallible_series(
                self.timeline.name().as_str(),
                &times,
                num_rows,
            )),
        ]
        .into_iter()
        .flatten() // filter options
        // One column for each component index.
        .chain(indices.iter().filter_map(|(component, comp_row_nrs)| {
            let datatype = find_component_datatype(store, component)?;
            component_as_series(store, num_rows, datatype, *component, comp_row_nrs).into()
        }));

        DataFrame::new(comp_series.collect::<Vec<_>>())
            // This cannot fail at this point, all series are guaranteed to have data and be of
            // same length.
            .unwrap()
    }
}

// ---

fn insert_ids_as_series(
    num_rows: usize,
    indices: &IntMap<ComponentName, SecondaryIndex>,
) -> Option<Series> {
    crate::profile_function!();

    indices.get(&DataStore::insert_id_key()).map(|insert_ids| {
        let insert_ids = insert_ids
            .iter()
            .map(|id| id.map(|id| id.0.get()))
            .collect::<Vec<_>>();
        let insert_ids = UInt64Array::from(insert_ids);
        new_infallible_series(DataStore::insert_id_key().as_str(), &insert_ids, num_rows)
    })
}

fn find_component_datatype(
    store: &DataStore,
    component: &ComponentName,
) -> Option<arrow2::datatypes::DataType> {
    crate::profile_function!();

    let timeless = store
        .timeless_components
        .get(component)
        .map(|table| table.datatype.clone());
    let temporal = store
        .components
        .get(component)
        .map(|table| table.datatype.clone());
    timeless.or(temporal)
}

fn component_as_series(
    store: &DataStore,
    num_rows: usize,
    datatype: arrow2::datatypes::DataType,
    component: ComponentName,
    comp_row_nrs: &[Option<RowIndex>],
) -> Series {
    crate::profile_function!();

    let components = &[component];

    // For each row in the index, grab the associated data from the component tables.
    let comp_rows: Vec<Option<_>> = comp_row_nrs
        .iter()
        .cloned()
        .map(|comp_row_nr| store.get(components, &[comp_row_nr])[0].clone())
        .collect();

    // Computing the validity bitmap is just a matter of checking whether the data was
    // available in the component tables.
    let comp_validity: Vec<_> = comp_rows.iter().map(|row| row.is_some()).collect();

    // Each cell is actually a list, so we need to compute offsets one cell at a time.
    let comp_lengths = comp_rows
        .iter()
        .map(|row| row.as_ref().map_or(0, |row| row.len()));

    let comp_values: Vec<_> = comp_rows.iter().flatten().map(|row| row.as_ref()).collect();

    // Bring everything together into one big list.
    let comp_values = ListArray::<i32>::new(
        ListArray::<i32>::default_datatype(datatype.clone()),
        Offsets::try_from_lengths(comp_lengths).unwrap().into(),
        // It's possible that all rows being referenced were already garbage collected (or simply
        // never existed to begin with), at which point `comp_rows` will be empty... and you can't
        // call `concatenate` on an empty list without panicking.
        if comp_values.is_empty() {
            new_empty_array(datatype)
        } else {
            concatenate(comp_values.as_slice()).unwrap().to_boxed()
        },
        Some(Bitmap::from(comp_validity)),
    );

    new_infallible_series(component.as_str(), &comp_values, num_rows)
}

// ---

fn new_infallible_series(name: &str, data: &dyn Array, len: usize) -> Series {
    crate::profile_function!();

    Series::try_from((name, data.as_ref().clean_for_polars())).unwrap_or_else(|_| {
        let errs = Utf8Array::<i32>::from(vec![Some("<ERR>"); len]);
        Series::try_from((name, errs.boxed())).unwrap() // cannot fail
    })
}

/// Sorts the columns of the given dataframe according to the following rules:
// - insert ID comes first if it's available,
// - followed by lexically sorted timelines,
// - followed by the entity path,
// - and finally all components in lexical order.
fn sort_df_columns(df: &DataFrame, store_insert_ids: bool) -> DataFrame {
    crate::profile_function!();

    let columns: Vec<_> = {
        let mut all = df.get_column_names();
        all.sort();

        all.remove(all.binary_search(&"entity").expect("has to exist"));

        if store_insert_ids {
            all.remove(
                all.binary_search(&DataStore::insert_id_key().as_str())
                    .expect("has to exist"),
            );
        }

        let timelines = all
            .iter()
            .copied()
            .filter(|name| !name.starts_with("rerun."))
            .map(Some)
            .collect::<Vec<_>>();

        let components = all
            .iter()
            .copied()
            .filter(|name| name.starts_with("rerun."))
            .map(Some)
            .collect::<Vec<_>>();

        [
            vec![store_insert_ids.then(|| DataStore::insert_id_key().as_str())],
            timelines,
            vec![Some("entity")],
            components,
        ]
        .into_iter()
        .flatten() // flatten vectors
        .flatten() // filter options
        .collect()
    };

    df.select(columns).unwrap()
}
