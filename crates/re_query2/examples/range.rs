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

    let mut resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline, TimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    let results: RangeResults = re_query2::range(
        &store,
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(), // no generics!
    );

    let all_points: &RangeComponentResults = results.get_required::<MyPoint>()?;
    let all_colors: &RangeComponentResults = results.get_optional::<MyColor>();
    let all_labels: &RangeComponentResults = results.get_optional::<MyLabel>();

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&mut resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&mut resolver)
    );
    let all_labels = izip!(
        all_labels.iter_indices(),
        all_labels.iter_sparse::<MyLabel>(&mut resolver)
    );

    let all_frames = range_zip_1x2(all_points, all_colors, all_labels);

    eprintln!("results:");
    for ((data_time, row_id), points, colors, labels) in all_frames {
        let points = match points.flatten() {
            PromiseResult::Pending => {
                continue; // or something else
            }
            PromiseResult::Ready(data) => data,
            PromiseResult::Error(err) => return Err(err.into()),
        };

        let colors = if let Some(colors) = colors {
            match colors.flatten() {
                PromiseResult::Pending => {
                    continue; // or something else
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
                    continue; // or something else
                }
                PromiseResult::Ready(data) => data,
                PromiseResult::Error(err) => return Err(err.into()),
            }
        } else {
            vec![]
        };
        let label_default_fn = || None;

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
        let timepoint = [build_frame_nr(123.into())];

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
        let timepoint = [build_frame_nr(456.into())];

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
