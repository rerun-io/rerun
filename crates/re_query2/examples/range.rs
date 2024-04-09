use itertools::{izip, Itertools};
use re_data_store::{DataStore, RangeQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, DataRow, RowId, TimeRange, TimeType, Timeline};
use re_types_core::{Archetype as _, Loggable as _};

use re_query2::{
    clamped_zip_1x2, range_zip_1x2, PromiseResolver, PromiseResult, RangeComponentResults,
    RangeResults,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{}", store.to_data_table()?);

    let resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline, TimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    // First, get the raw results for this query.
    //
    // Raw here means that these results are neither deserialized, nor resolved/converted.
    // I.e. this corresponds to the raw `DataCell`s, straight from our datastore.
    let results: RangeResults = re_query2::range(
        &store,
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(), // no generics!
    );

    // Then, grab the raw results for each individual components.
    //
    // This is still raw data, but now a choice has been made regarding the nullability of the
    // _component batch_ itself (that says nothing about its _instances_!).
    //
    // * `get_required` returns an error if the component batch is missing
    // * `get_or_empty` returns an empty set of results if the component if missing
    // * `get` returns an option
    let all_points: &RangeComponentResults = results.get_required(MyPoint::name())?;
    let all_colors: &RangeComponentResults = results.get_or_empty(MyColor::name());
    let all_labels: &RangeComponentResults = results.get_or_empty(MyLabel::name());

    let all_indexed_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_indexed_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );
    let all_indexed_labels = izip!(
        all_labels.iter_indices(),
        all_labels.iter_sparse::<MyLabel>(&resolver)
    );

    let all_frames = range_zip_1x2(all_indexed_points, all_indexed_colors, all_indexed_labels);

    // Then comes the time to resolve/convert and deserialize the data, _for each timestamp_.
    // These steps have to be done together for efficiency reasons.
    //
    // Both the resolution and deserialization steps might fail, which is why this returns a `Result<Result<T>>`.
    // Use `PromiseResult::flatten` to simplify it down to a single result.
    //
    // A choice now has to be made regarding the nullability of the _component batch's instances_.
    // Our IDL doesn't support nullable instances at the moment -- so for the foreseeable future you probably
    // shouldn't be using anything but `iter_dense`.
    eprintln!("results:");
    for ((data_time, row_id), points, colors, labels) in all_frames {
        let points = match points.flatten() {
            PromiseResult::Pending => {
                // Handle the fact that the data isn't ready appropriately.
                continue;
            }
            PromiseResult::Ready(data) => data,
            PromiseResult::Error(err) => return Err(err.into()),
        };

        let colors = if let Some(colors) = colors {
            match colors.flatten() {
                PromiseResult::Pending => {
                    // Handle the fact that the data isn't ready appropriately.
                    continue;
                }
                PromiseResult::Ready(data) => data,
                PromiseResult::Error(err) => return Err(err.into()),
            }
        } else {
            vec![]
        };
        let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

        let labels = if let Some(labels) = labels {
            match labels.flatten() {
                PromiseResult::Pending => {
                    // Handle the fact that the data isn't ready appropriately.
                    continue;
                }
                PromiseResult::Ready(data) => data,
                PromiseResult::Error(err) => return Err(err.into()),
            }
        } else {
            vec![]
        };
        let label_default_fn = || None;

        // With the data now fully resolved/converted and deserialized, the joining logic can be
        // applied.
        //
        // In most cases this will be either a clamped zip, or no joining at all.

        let results = clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn)
            .collect_vec();
        eprintln!("{data_time:?} @ {row_id}:\n    {results:?}");
    }

    Ok(())
}

// ---

fn store() -> anyhow::Result<DataStore> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        re_types::components::InstanceKey::name(),
        Default::default(),
    );

    let entity_path = "points";

    {
        let timepoint = [build_frame_nr(123)];

        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
        store.insert_row(&row)?;

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let labels = vec![MyLabel("a".into()), MyLabel("b".into())];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, labels)?;
        store.insert_row(&row)?;
    }

    {
        let timepoint = [build_frame_nr(456)];

        let colors = vec![MyColor::from_rgb(255, 0, 0), MyColor::from_rgb(0, 0, 255)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let points = vec![
            MyPoint::new(10.0, 20.0),
            MyPoint::new(30.0, 40.0),
            MyPoint::new(50.0, 60.0),
        ];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
        store.insert_row(&row)?;
    }

    Ok(store)
}
