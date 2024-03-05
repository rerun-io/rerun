use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::EntityPath;
use re_types_core::ComponentName;

use crate::LatestAtResults;

// ---

// TODO
// TODO: each component is considered a primary (queried from its own PoV)
pub fn latest_at(
    store: &DataStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> LatestAtResults {
    re_tracing::profile_function!(entity_path.to_string());

    let mut results = LatestAtResults::default();

    for component_name in component_names {
        let Some((time, row_id, mut cells)) =
            store.latest_at(query, entity_path, component_name, &[component_name])
        else {
            continue;
        };

        // - `cells[0]` is guaranteed to exist since we passed `&[component_name]`
        // - `cells[0]` is guaranteed to be non-null, otherwise the whole result would be null
        let cell = cells[0].take().unwrap();

        results.add(component_name, (time, row_id), cell);
    }

    results
}

// TODO
#[cfg(test)]
mod tests {
    use nohash_hasher::IntSet;
    use re_data_store::DataStore;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{build_frame_nr, DataRow, RowId};
    use re_types_core::{Archetype as _, Loggable as _};

    use crate::{clamped_zip_1x1, PromiseResolver};

    use super::*;

    #[test]
    fn basics() -> anyhow::Result<()> {
        let mut resolver = PromiseResolver::default();

        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            re_types::components::InstanceKey::name(),
            Default::default(),
        );

        let ent_path = "point";
        let timepoint = [build_frame_nr(123.into())];

        // Create some points with implicit instances
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, points)?;
        store.insert_row(&row)?;

        // Assign one of them a color with an explicit instance
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let timeline_query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
        let mut results = crate::latest_at(
            &store,
            &timeline_query,
            &ent_path.into(),
            MyPoints::all_components().iter().cloned(),
        );

        // TODO: the final example needs at least 2 optional comps
        // TODO: add full type annotations to examples
        // TODO: "lower level -- it's all about escape hatches!"

        {
            let expected_components: IntSet<ComponentName> =
                [MyPoint::name(), MyColor::name()].into_iter().collect();
            let got_components: IntSet<ComponentName> =
                results.components.keys().copied().collect();
            similar_asserts::assert_eq!(expected_components, got_components);

            let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
            let expected_colors = vec![
                Some(MyColor::from_rgb(255, 0, 0)),
                Some(MyColor::from_rgb(255, 0, 0)),
            ];

            let points = results.get_required::<MyPoint>().unwrap();
            let point_data = match points.iter_dense::<MyPoint>(&mut resolver).flatten() {
                crate::PromiseResult::Pending => {
                    // Come back next frame.
                    return Ok(());
                }
                crate::PromiseResult::Ready(data) => data,
                crate::PromiseResult::Error(err) => return Err(err.into()),
            };

            let colors = results.get_optional::<MyColor>();
            let color_data = match colors.iter_sparse::<MyColor>(&mut resolver).flatten() {
                crate::PromiseResult::Pending => {
                    // Come back next frame.
                    return Ok(());
                }
                crate::PromiseResult::Ready(data) => data,
                crate::PromiseResult::Error(err) => return Err(err.into()),
            };
            let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

            let (got_points, got_colors): (Vec<_>, Vec<_>) =
                clamped_zip_1x1(point_data, color_data, color_default_fn).unzip();

            similar_asserts::assert_eq!(expected_points, got_points);
            similar_asserts::assert_eq!(expected_colors, got_colors);
        }

        Ok(())
    }
}
