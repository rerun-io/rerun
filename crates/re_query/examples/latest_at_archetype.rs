use itertools::Itertools;
use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{build_frame_nr, DataRow, RowId, TimeType, Timeline};
use re_types::{
    archetypes::Points2D,
    components::{Color, Position2D, Text},
};
use re_types_core::{Archetype as _, Loggable as _};

use re_query::{clamped_zip_1x2, CachedLatestAtResults, PromiseResolver, PromiseResult};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{}", store.to_data_table()?);

    let resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    let caches = re_query::Caches::new(&store);

    // First, get the results for this query.
    //
    // They might or might not already be cached. We won't know for sure until we try to access
    // each individual component's data below.
    let results: CachedLatestAtResults = caches.latest_at(
        &store,
        &query,
        &entity_path.into(),
        Points2D::all_components().iter().cloned(), // no generics!
    );

    // Then make use of the `ToArchetype` helper trait in order to query, resolve, deserialize and
    // cache an entire archetype all at once.
    use re_query::ToArchetype as _;

    let arch: Points2D = match results.to_archetype(&resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(arch) => arch,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    // With the data now fully resolved/converted and deserialized, some joining logic can be
    // applied if desired.
    //
    // In most cases this will be either a clamped zip, or no joining at all.

    let color_default_fn = || None;
    let label_default_fn = || None;

    let results = clamped_zip_1x2(
        arch.positions.iter(),
        arch.colors
            .iter()
            .flat_map(|colors| colors.iter().map(Some)),
        color_default_fn,
        arch.labels
            .iter()
            .flat_map(|labels| labels.iter().map(Some)),
        label_default_fn,
    )
    .collect_vec();

    eprintln!("results:\n{results:?}");

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

        let points = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
        store.insert_row(&row)?;

        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let labels = vec![Text("a".into()), Text("b".into())];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, labels)?;
        store.insert_row(&row)?;
    }

    Ok(store)
}
