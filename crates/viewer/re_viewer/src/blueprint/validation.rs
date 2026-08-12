use re_entity_db::EntityDb;
use re_types_core::reflection::ComponentReflectionMap;

/// Because blueprints are both read and written, their schema must match what we
/// expect to find, or else we will run into all kinds of problems.
///
/// A persisted blueprint can have been written by an older version of the viewer, in which
/// case some of its components may use datatypes we no longer understand.
/// Reading those would silently discard data (and debug-panic), so we reject the whole
/// blueprint instead and start over from a fresh one.
///
/// This checks _all_ components in the blueprint, not just the blueprint-specific ones:
/// blueprints also store overrides and defaults for regular data components.
pub fn is_valid_blueprint(
    blueprint: &EntityDb,
    component_reflection: &ComponentReflectionMap,
) -> bool {
    re_tracing::profile_function!();

    let engine = blueprint.storage_engine();

    // Collect _all_ mismatches, so that a single log message explains everything that is wrong.
    let mut mismatches = vec![];

    for (entity_path, column) in engine.schema().all_column_metadata() {
        let Some(component_type) = column.descriptor.component_type else {
            // Untyped component: there is nothing to check it against.
            continue;
        };
        let Some(reflection) = component_reflection.get(&component_type) else {
            // A component we no longer know about. The viewer will ignore it anyway.
            continue;
        };

        if column.datatype != reflection.datatype {
            mismatches.push(format!(
                "  {} of {entity_path}: found {}, expected {}",
                column.descriptor, column.datatype, reflection.datatype,
            ));
        }
    }

    if mismatches.is_empty() {
        true
    } else {
        mismatches.sort();
        re_log::warn_once!(
            "Blueprint has {} component(s) with unexpected datatypes:\n{}",
            mismatches.len(),
            mismatches.join("\n"),
        );
        false
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{Float32Array, Float64Array};

    use re_chunk::{Chunk, RowId};
    use re_entity_db::EntityDb;
    use re_log_types::{StoreId, TimePoint};
    use re_sdk_types::{Loggable as _, archetypes::Points3D, components::Radius};

    use super::is_valid_blueprint;

    fn reflection() -> re_types_core::reflection::ComponentReflectionMap {
        re_sdk_types::reflection::generate_reflection()
            .expect("failed to generate reflection")
            .components
    }

    /// One `Radius` column per given array, each on its own entity,
    /// as if logged as overrides in a blueprint.
    fn blueprint_with_radius_columns(
        arrays: impl IntoIterator<Item = arrow::array::ArrayRef>,
    ) -> EntityDb {
        let mut blueprint = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Blueprint,
            "test_app",
        ));

        for (i, array) in arrays.into_iter().enumerate() {
            let chunk = Chunk::builder(format!("/view/some-view/overrides/entity-{i}").as_str())
                .with_row(
                    RowId::new(),
                    TimePoint::default(),
                    [(Points3D::descriptor_radii(), array)],
                )
                .build()
                .expect("failed to build chunk");

            blueprint
                .add_chunk(&Arc::new(chunk))
                .expect("failed to add chunk");
        }

        blueprint
    }

    fn blueprint_with_radius_column(array: arrow::array::ArrayRef) -> EntityDb {
        blueprint_with_radius_columns([array])
    }

    #[test]
    fn empty_blueprint_is_valid() {
        let component_reflection = reflection();
        let blueprint = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Blueprint,
            "test_app",
        ));
        assert!(is_valid_blueprint(&blueprint, &component_reflection));
    }

    #[test]
    fn matching_datatype_is_valid() {
        let component_reflection = reflection();
        assert_eq!(
            Radius::arrow_datatype(),
            arrow::datatypes::DataType::Float32
        );

        let blueprint = blueprint_with_radius_column(Arc::new(Float32Array::from(vec![1.0])));
        assert!(is_valid_blueprint(&blueprint, &component_reflection));
    }

    /// A blueprint written before a component changed its datatype must be rejected,
    /// even for non-blueprint components (which are used for overrides and defaults).
    #[test]
    fn mismatched_datatype_is_invalid() {
        let component_reflection = reflection();

        let blueprint = blueprint_with_radius_column(Arc::new(Float64Array::from(vec![1.0])));
        assert!(!is_valid_blueprint(&blueprint, &component_reflection));
    }

    /// A mismatch on any column invalidates the blueprint, not just the first one visited.
    #[test]
    fn mismatch_after_valid_column_is_invalid() {
        let component_reflection = reflection();

        let blueprint = blueprint_with_radius_columns([
            Arc::new(Float32Array::from(vec![1.0])) as arrow::array::ArrayRef,
            Arc::new(Float64Array::from(vec![1.0])),
        ]);
        assert!(!is_valid_blueprint(&blueprint, &component_reflection));
    }

    #[test]
    fn multiple_mismatches_are_invalid() {
        let component_reflection = reflection();

        let blueprint = blueprint_with_radius_columns([
            Arc::new(Float64Array::from(vec![1.0])) as arrow::array::ArrayRef,
            Arc::new(Float64Array::from(vec![2.0])),
        ]);
        assert!(!is_valid_blueprint(&blueprint, &component_reflection));
    }
}
