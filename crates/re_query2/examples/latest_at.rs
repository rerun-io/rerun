use itertools::Itertools;
use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, DataRow, RowId, TimeType, Timeline};
use re_types_core::{Archetype as _, Loggable as _};

use re_query2::{
    clamped_zip_1x2, LatestAtComponentResults, LatestAtResults, PromiseResolver, PromiseResult,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{}", store.to_data_table()?);

    let mut resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    // First, get the raw results for this query.
    //
    // Raw here means that these results are neither deserialized, nor resolved/converted.
    // I.e. this corresponds to the raw `DataCell`s, straight from our datastore.
    let results: LatestAtResults = re_query2::latest_at(
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
    // * `get_optional` returns an empty set of results if the component if missing
    // * `get` returns an option
    let points: &LatestAtComponentResults = results.get_required::<MyPoint>()?;
    let colors: &LatestAtComponentResults = results.get_optional::<MyColor>();
    let labels: &LatestAtComponentResults = results.get_optional::<MyLabel>();

    // Then comes the time to resolve/convert and deserialize the data.
    // These steps have to be done together for efficiency reasons.
    //
    // Both the resolution and deserialization steps might fail, which is why this returns a `Result<Result<T>>`.
    // Use `PromiseResult::flatten` to simplify it down to a single result.
    //
    // A choice now has to be made regarding the nullability of the _component batch's instances_.
    // Our IDL doesn't support nullable instances at the moment -- so for the foreseeable future you probably
    // shouldn't be using anything but `iter_dense`.

    let points = match points.iter_dense::<MyPoint>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    let colors = match colors.iter_dense::<MyColor>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    let labels = match labels.iter_sparse::<MyLabel>(&mut resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    // With the data now fully resolved/converted and deserialized, the joining logic can be
    // applied.
    //
    // In most cases this will be either a clamped zip, or no joining at all.

    let color_default_fn = || MyColor::from(0xFF00FFFF);
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

    Ok(store)
}
