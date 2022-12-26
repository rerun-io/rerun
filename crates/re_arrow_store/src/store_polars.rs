use arrow2::{
    array::{new_empty_array, Array, ListArray, UInt64Array, Utf8Array},
    bitmap::Bitmap,
    buffer::Buffer,
    compute::concatenate::concatenate,
};
use nohash_hasher::IntMap;
use polars_core::{functions::diag_concat_df, prelude::*};
use re_log_types::ComponentName;

use crate::{store::ComponentTable, DataStoreConfig, IndexBucket};

use super::{DataStore, IndexBucketIndices};

impl DataStore {
    /// Dumps the entire datastore as a flat, denormalized dataframe.
    ///
    /// This cannot fail: it always tries to yield as much valuable information as it can, even in
    /// the face of errors.
    pub fn to_dataframe(&self) -> DataFrame {
        let dfs: Vec<DataFrame> = self
            .indices
            .values()
            .map(|index| {
                let dfs: Vec<_> = index
                    .buckets
                    .values()
                    .map(|bucket| (index.ent_path.clone(), bucket))
                    .map(|(ent_path, bucket)| {
                        let mut df = bucket.to_dataframe(&self.config, &self.components);
                        let nb_rows = df.get_columns()[0].len();

                        // Add a column where every row is the entity path.
                        let entities = {
                            let ent_path = ent_path.to_string();
                            let ent_path = Some(ent_path.as_str());
                            let entities = Utf8Array::<i32>::from(vec![ent_path; nb_rows]).boxed();
                            new_infallible_series("entity", entities, nb_rows)
                        };
                        let df = df.with_column(entities).unwrap(); // cannot fail

                        df.clone()
                    })
                    .collect();

                // Concatenate all buckets of the index together.
                //
                // This has to be done diagonally since each bucket can and will have different
                // numbers of columns (== components) and rows.
                diag_concat_df(dfs.as_slice())
                    // TODO(cmc): is there any way this can fail in this case?
                    .unwrap()
            })
            .collect();

        // Concatenate all indices together.
        //
        // This has to be done diagonally since these indices refer to different entities with
        // potentially wildly different sets of components and lengths.
        let df = diag_concat_df(&dfs)
            // TODO(cmc): is there any way this can fail in this case?
            .unwrap();

        // Here's the order we're going for:
        // - insert ID comes first if it's available,
        // - followed by lexically sorted timelines,
        // - followed by the entity path,
        // - and finally all components in lexical order.
        let columns: Vec<_> = {
            let mut all = df.get_column_names();
            all.sort();

            all.remove(all.binary_search(&"entity").expect("has to exist"));

            if self.config.store_insert_ids {
                all.remove(
                    all.binary_search(&Self::insert_id_key().as_str())
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
                vec![self
                    .config
                    .store_insert_ids
                    .then(|| Self::insert_id_key().as_str())],
                timelines,
                vec![Some("entity")],
                components,
            ]
            .into_iter()
            .flatten() // flatten vectors
            .flatten() // filter options
            .collect()
        };

        if self.config.store_insert_ids {
            // If insert IDs are available, sort by them.
            df.sort(vec![Self::insert_id_key().as_str()], vec![false])
                .and_then(|df| df.select(columns))
                .unwrap()
        } else {
            df.select(columns).unwrap()
        }
    }
}

impl IndexBucket {
    /// Dumps the entire bucket as a flat, denormalized dataframe.
    ///
    /// This cannot fail: it always tries to yield as much valuable information as it can, even in
    /// the face of errors.
    pub fn to_dataframe(
        &self,
        config: &DataStoreConfig,
        components: &IntMap<ComponentName, ComponentTable>,
    ) -> DataFrame {
        let (_, times) = self.times();
        let nb_rows = times.len();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times: _,
            indices,
        } = &*self.indices.read();

        let insert_ids = config
            .store_insert_ids
            .then(|| {
                indices.get(&DataStore::insert_id_key()).map(|insert_ids| {
                    let insert_ids = insert_ids
                        .iter()
                        .map(|id| id.map(|id| id.0.get()))
                        .collect::<Vec<_>>();
                    let insert_ids = UInt64Array::from(insert_ids);
                    new_infallible_series(
                        DataStore::insert_id_key().as_str(),
                        insert_ids.boxed(),
                        nb_rows,
                    )
                })
            })
            .flatten();

        // Need to create one `Series` for the time index and one for each component index.
        let comp_series = [
            // One column for insert IDs, if they are available.
            insert_ids,
            // One column for the time index.
            Some(new_infallible_series(
                self.timeline.name().as_str(),
                times.boxed(),
                nb_rows,
            )),
        ]
        .into_iter()
        .flatten() // filter options
        // One column for each component index.
        .chain(indices.iter().filter_map(|(component, comp_row_nrs)| {
            let comp_table = components.get(component)?;

            // For each row in the index, grab the associated data from the component tables.
            let comp_rows: Vec<Option<_>> = comp_row_nrs
                .iter()
                .map(|comp_row_nr| comp_row_nr.and_then(|comp_row_nr| comp_table.get(comp_row_nr)))
                .collect();

            // Computing the validity bitmap is just a matter of checking whether the data was
            // available in the component tables.
            let comp_validity: Vec<_> = comp_rows.iter().map(|row| row.is_some()).collect();

            // We have to build empty arrays where data is missing, otherwise we'd have a length
            // mismatch between the validity bitmap and the data, which results in an error when
            // building the final ListArray.
            let comp_values: Vec<_> = comp_rows
                .into_iter()
                .map(|row| row.unwrap_or_else(|| new_empty_array(comp_table.datatype.clone())))
                .collect();
            let comp_values: Vec<_> = comp_values.iter().map(|arr| &**arr).collect();

            // Each cell is actually a list, so we need to compute offsets one cell at a time.
            let mut offset = 0i32;
            let comp_offsets: Vec<_> = comp_values
                .iter()
                .map(|row| {
                    let ret = offset;
                    offset += row.len() as i32;
                    ret
                })
                // don't forget the last element!
                .chain(std::iter::once(
                    comp_values.iter().map(|row| row.len() as i32).sum(),
                ))
                .collect();

            // Bring everything together into of big list.
            let comp_values = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(comp_table.datatype.clone()),
                Buffer::from(comp_offsets),
                concatenate(comp_values.as_slice()).unwrap().to_boxed(),
                Some(Bitmap::from(comp_validity)),
            )
            .boxed();

            Some(new_infallible_series(
                component.as_str(),
                comp_values,
                nb_rows,
            ))
        }));

        DataFrame::new(comp_series.collect::<Vec<_>>())
            // This cannot fail at this point, all series are guaranteed to have data and be of
            // same length.
            .unwrap()
    }
}

fn new_infallible_series(name: &str, data: Box<dyn Array>, len: usize) -> Series {
    Series::try_from((name, data)).unwrap_or_else(|_| {
        let errs = Utf8Array::<i32>::from(
            std::iter::repeat(Some("<ERR>"))
                .take(len)
                .collect::<Vec<_>>(),
        );
        Series::try_from((name, errs.boxed())).unwrap() // cannot fail
    })
}
