use itertools::Itertools;
use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, DataRow, RowId, TimeType, Timeline};
use re_types_core::{Archetype as _, Loggable as _};

use re_query_cache2::{
    clamped_zip_1x2, CachedLatestAtComponentResults, CachedLatestAtResults, PromiseResolver,
    PromiseResult,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{}", store.to_data_table()?);

    let caches = re_query_cache2::Caches::new(&store);

    let mut resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    let results: CachedLatestAtResults = caches.latest_at(
        &store,
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(), // no generics!
    );

    let points: &CachedLatestAtComponentResults = results.get_required::<MyPoint>()?;
    let colors: &CachedLatestAtComponentResults = results.get_optional::<MyColor>();
    let labels: &CachedLatestAtComponentResults = results.get_optional::<MyLabel>();

    let points = match points.iter_dense::<MyPoint>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Come back next frame.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    let colors = match colors.to_dense::<MyColor>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Come back next frame.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };
    let color_default_fn = || {
        static DEFAULT: MyColor = MyColor(0xFF00FFFF);
        &DEFAULT
    };

    let labels = match labels.iter_sparse::<MyLabel>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Come back next frame.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };
    let label_default_fn = || None;

    let results =
        clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn).collect_vec();
    eprintln!("results:\n{results:#?}");

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

    Ok(store)
}
