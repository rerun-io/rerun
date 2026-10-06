use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{ArrayRef, new_empty_array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::error::ArrowError;
use serde_json::{Map, Value};

use re_chunk::{Chunk, ChunkError, RowId};
use re_chunk_store::ChunkStore;
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::reflection::{ArchetypeReflection, ComponentReflectionMap, Reflection};
use re_sdk_types::{ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType};

use crate::archetype_key::archetype_by_json_key;
use crate::arrow_union::array_from_json;
use crate::special::special_batch_from_json;

/// Why one JSON value could not be decoded into a component.
#[derive(Debug, thiserror::Error)]
pub enum ValueFromJsonError {
    #[error("expected {expected}, got {got}")]
    Expected { expected: String, got: String },

    #[error("unknown variant {name:?}; expected one of: {expected}")]
    UnknownEnumVariant { name: String, expected: String },

    #[error("unknown field {name:?}; expected one of: {expected}")]
    UnknownField { name: String, expected: String },

    #[error("unknown archetype {name:?}; expected one of: {expected}")]
    UnknownArchetype { name: String, expected: String },

    #[error("ambiguous archetype {name:?}; use the full name of one of: {candidates}")]
    AmbiguousArchetype { name: String, candidates: String },

    #[error("unknown component type {0}")]
    UnknownComponentType(ComponentType),

    #[error(
        "expected a value of Arrow type {datatype}, or an array of them; got {nesting} levels of arrays"
    )]
    Nesting { datatype: DataType, nesting: usize },

    #[error("decoding {datatype} from JSON is not supported, since it holds a union")]
    UnsupportedDatatype { datatype: DataType },

    /// `arrow-json` could not decode the values as `datatype`.
    #[error("could not decode as Arrow type {datatype}: {err}")]
    Decode {
        datatype: DataType,
        #[source]
        err: ArrowError,
    },

    /// The decoded parts could not be assembled into an Arrow array of `datatype`.
    #[error("could not build an Arrow array of type {datatype}: {err}")]
    BuildArray {
        datatype: DataType,
        #[source]
        err: ArrowError,
    },

    #[error("could not encode as a batch of {component_type}: {err}")]
    ToArrow {
        component_type: ComponentType,
        #[source]
        err: re_sdk_types::SerializationError,
    },

    #[error("not a valid batch of {component_type}: {err}")]
    Invalid {
        component_type: ComponentType,
        #[source]
        err: re_sdk_types::DeserializationError,
    },
}

impl ValueFromJsonError {
    /// `got` was not the `expected` kind of value.
    pub fn expected(expected: impl Into<String>, got: &Value) -> Self {
        // Enough to recognize the value, without quoting a whole blueprint.
        const MAX_LEN: usize = 100;

        let mut got = got.to_string();
        if got.len() > MAX_LEN {
            got.truncate(got.floor_char_boundary(MAX_LEN));
            got.push('…');
        }

        Self::Expected {
            expected: expected.into(),
            got,
        }
    }
}

/// Why JSON could not be turned into chunks.
#[derive(Debug, thiserror::Error)]
pub enum ChunksFromJsonError {
    #[error("expected a JSON object keyed by entity path, got {got}")]
    NotAnObject { got: String },

    /// `err`, at `location`: the entity path, archetype and field it is in, joined by ` → `.
    #[error("{location}: {err}")]
    At {
        location: String,
        err: ValueFromJsonError,
    },

    #[error("{entity_path}: {err}")]
    Chunk {
        entity_path: EntityPath,
        err: ChunkError,
    },
}

/// The components of one entity's row, in the form `Chunk::builder` takes.
///
/// Keyed by component identifier rather than by descriptor, so that a component the JSON writes
/// is not also cleared under the descriptor `store` has for it.
type Row = BTreeMap<ComponentIdentifier, (ComponentDescriptor, ArrayRef)>;

/// The chunks that replace everything in `store` with `json`, written at `timepoint`.
///
/// The JSON is in the form the crate docs describe and [`crate::json_from_store`] reads. A field
/// or entity it leaves out is cleared.
///
/// `store` is only read, for what to clear.
pub fn chunks_from_json(
    json: &Value,
    store: &ChunkStore,
    timepoint: &TimePoint,
    reflection: &Reflection,
) -> Result<Vec<Chunk>, ChunksFromJsonError> {
    re_tracing::profile_function!();

    let Value::Object(entities) = json else {
        return Err(ChunksFromJsonError::NotAnObject {
            got: json.to_string(),
        });
    };

    let mut rows: BTreeMap<EntityPath, Row> = BTreeMap::new();

    for (entity_path, archetypes) in entities {
        let entity_path = EntityPath::parse_forgiving(entity_path);
        let row = rows.entry(entity_path.clone()).or_default();
        for (descriptor, batch) in components_from_json(reflection, &entity_path, archetypes)? {
            row.insert(descriptor.component, (descriptor, batch));
        }
    }

    for (entity_path, entry) in store.schema().all_column_metadata() {
        if entry.datatype == DataType::Null {
            continue;
        }
        rows.entry(entity_path.clone())
            .or_default()
            .entry(entry.descriptor.component)
            .or_insert_with(|| (entry.descriptor.clone(), new_empty_array(&entry.datatype)));
    }

    rows.into_iter()
        .filter(|(_, components)| !components.is_empty())
        .map(|(entity_path, components)| {
            Chunk::builder(entity_path.clone())
                .with_row(RowId::new(), timepoint.clone(), components.into_values())
                .build()
                .map_err(|err| ChunksFromJsonError::Chunk { entity_path, err })
        })
        .collect()
}

/// The components of one entity, from its object of archetypes.
fn components_from_json(
    reflection: &Reflection,
    entity_path: &EntityPath,
    archetypes: &Value,
) -> Result<Vec<(ComponentDescriptor, ArrayRef)>, ChunksFromJsonError> {
    let at = |err| ChunksFromJsonError::At {
        location: entity_path.to_string(),
        err,
    };

    let Value::Object(archetypes) = archetypes else {
        return Err(at(ValueFromJsonError::expected(
            "an object keyed by archetype name",
            archetypes,
        )));
    };

    let mut components = Vec::new();
    for (archetype, fields) in archetypes {
        let (archetype_name, archetype_reflection) =
            archetype_by_json_key(reflection, archetype).map_err(at)?;
        components.extend(archetype_components_from_json(
            reflection,
            &format!("{entity_path} → {archetype}"),
            archetype_name,
            archetype_reflection,
            fields,
        )?);
    }
    Ok(components)
}

/// The components of one archetype, from its object of fields.
///
/// `location` is where the archetype is, for errors: `{entity_path} → {archetype}`.
fn archetype_components_from_json(
    reflection: &Reflection,
    location: &str,
    archetype_name: ArchetypeName,
    archetype_reflection: &ArchetypeReflection,
    fields: &Value,
) -> Result<Vec<(ComponentDescriptor, ArrayRef)>, ChunksFromJsonError> {
    let Value::Object(fields) = fields else {
        return Err(ChunksFromJsonError::At {
            location: location.to_owned(),
            err: ValueFromJsonError::expected("an object keyed by field name", fields),
        });
    };

    let mut components = Vec::with_capacity(fields.len());
    for (field, value) in fields {
        let Some(field_reflection) = archetype_reflection.field_by_name(field) else {
            return Err(ChunksFromJsonError::At {
                location: location.to_owned(),
                err: ValueFromJsonError::UnknownField {
                    name: field.clone(),
                    expected: archetype_reflection
                        .fields
                        .iter()
                        .map(|field| field.name)
                        .collect::<Vec<_>>()
                        .join(", "),
                },
            });
        };
        let descriptor = field_reflection.component_descriptor(archetype_name);
        let batch = verified_batch_from_json(
            &descriptor,
            field_reflection.component_type,
            value,
            &reflection.components,
        )
        .map_err(|err| ChunksFromJsonError::At {
            location: format!("{location} → {field}"),
            err,
        })?;
        components.push((descriptor, batch));
    }
    Ok(components)
}

/// One batch of `component_type`, checked to hold what that component expects.
fn verified_batch_from_json(
    descriptor: &ComponentDescriptor,
    component_type: ComponentType,
    value: &Value,
    components: &ComponentReflectionMap,
) -> Result<ArrayRef, ValueFromJsonError> {
    let component = components
        .get(&component_type)
        .ok_or(ValueFromJsonError::UnknownComponentType(component_type))?;
    let batch = batch_from_json(descriptor, &component.datatype, value, components)?;
    (component.verify_arrow_array)(batch.as_ref()).map_err(|err| ValueFromJsonError::Invalid {
        component_type,
        err,
    })?;
    Ok(batch)
}

/// One component batch of `datatype`, from a single instance or an array of them.
/// `null` clears it.
fn batch_from_json(
    descriptor: &ComponentDescriptor,
    datatype: &DataType,
    value: &Value,
    components: &ComponentReflectionMap,
) -> Result<ArrayRef, ValueFromJsonError> {
    if value.is_null() {
        return Ok(new_empty_array(datatype));
    }

    // A special form is never itself an array, so any array is a batch of them.
    let special_instances = match value {
        Value::Array(instances) => instances.as_slice(),
        _ => std::slice::from_ref(value),
    };
    if let Some(batch) = special_batch_from_json(descriptor, special_instances, components) {
        return batch;
    }

    array_from_json(datatype, instances_of(value, datatype)?)
}

/// Whether `value` is one instance of `datatype` or an array of them, told apart by how deeply
/// its arrays nest: `[1, 2, 3]` is one `Vec3D`, but three `Float32`s.
fn instances_of<'v>(
    value: &'v Value,
    datatype: &DataType,
) -> Result<&'v [Value], ValueFromJsonError> {
    let expected = list_nesting(datatype);
    let nesting = json_nesting(value);
    if nesting == expected {
        Ok(std::slice::from_ref(value))
    } else if let (Value::Array(instances), true) = (value, nesting == expected + 1) {
        Ok(instances)
    } else {
        Err(ValueFromJsonError::Nesting {
            datatype: datatype.clone(),
            nesting,
        })
    }
}

/// How many levels of lists one value of `datatype` is, e.g. 1 for a `Vec3D`.
fn list_nesting(datatype: &DataType) -> usize {
    match datatype {
        DataType::List(field) | DataType::LargeList(field) | DataType::FixedSizeList(field, _) => {
            1 + list_nesting(field.data_type())
        }
        _ => 0,
    }
}

/// How many levels of arrays `value` is, following the first element.
///
/// An empty array is one level, whatever it would have held.
fn json_nesting(value: &Value) -> usize {
    match value {
        Value::Array(elements) => 1 + elements.first().map_or(0, json_nesting),
        _ => 0,
    }
}

/// `instances` decoded by `arrow-json` into an array of `datatype`, which holds no union: see
/// [`array_from_json`] for one that does.
pub fn arrow_from_json(
    datatype: &DataType,
    instances: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    const COLUMN: &str = "value";

    let decode_error = |err| ValueFromJsonError::Decode {
        datatype: datatype.clone(),
        err,
    };

    let schema = Schema::new_with_metadata(
        vec![Field::new(COLUMN, datatype.clone(), true)],
        Default::default(),
    );
    let mut decoder = arrow::json::ReaderBuilder::new(Arc::new(schema))
        .with_strict_mode(true)
        .with_batch_size(instances.len().max(1))
        .build_decoder()
        .map_err(decode_error)?;

    let rows: Vec<Map<String, Value>> = instances
        .iter()
        .map(|instance| Map::from_iter([(COLUMN.to_owned(), instance.clone())]))
        .collect();
    decoder.serialize(&rows).map_err(decode_error)?;

    Ok(match decoder.flush().map_err(decode_error)? {
        Some(record_batch) => record_batch.column(0).clone(),
        None => new_empty_array(datatype),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use re_chunk_store::LatestAtQuery;
    use re_log_types::TimelineName;

    use super::*;
    use crate::test_util::{empty_blueprint, example_blueprint, read, write};

    /// Written back into the blueprint it was read from, as an agent does.
    #[test]
    fn a_blueprint_round_trips() {
        let mut blueprint = example_blueprint();
        let json = read(&blueprint);

        write(&mut blueprint, &json);

        assert_eq!(read(&blueprint), json);
    }

    /// Every field of every blueprint archetype survives JSON and back, so a new one cannot
    /// silently lose data.
    #[test]
    fn every_blueprint_archetype_round_trips() {
        let reflection = re_sdk_types::reflection::reflection();
        let mut original = empty_blueprint();
        let mut fields_without_placeholder = Vec::new();

        let mut archetypes: Vec<_> = reflection
            .archetypes
            .iter()
            .filter(|(_, archetype)| archetype.scope == Some("blueprint"))
            .collect();
        archetypes.sort_by_key(|(name, _)| name.as_str());
        assert!(
            !archetypes.is_empty(),
            "there should be blueprint archetypes"
        );

        for (name, archetype) in archetypes {
            let components = archetype.fields.iter().filter_map(|field| {
                let component = reflection.components.get(&field.component_type)?;
                let Some(placeholder) = component.custom_placeholder.clone() else {
                    fields_without_placeholder.push(format!(
                        "{}:{} ({})",
                        name.short_name(),
                        field.name,
                        component.datatype
                    ));
                    return None;
                };
                Some((field.component_descriptor(*name), placeholder))
            });
            let chunk = Chunk::builder(format!("archetypes/{}", name.short_name()))
                .with_row(RowId::new(), TimePoint::default(), components)
                .build()
                .unwrap();
            original.add_chunk(&Arc::new(chunk)).unwrap();
        }

        let json = read(&original);
        let mut copy = empty_blueprint();
        write(&mut copy, &json);
        assert_eq!(read(&copy), json);

        // Keep this list short: a field without a placeholder is not covered above.
        insta::assert_debug_snapshot!(fields_without_placeholder);
    }

    /// The fields [`every_blueprint_archetype_round_trips`] has no placeholder value for.
    #[test]
    fn fields_without_placeholder_round_trip() {
        let json = json!({
            "/card": { "CardLayout": { "field_order": ["a", "b"], "link": "l", "title": "t" } },
            "/eye": { "EyeControls3D": { "speed": 2.5 } },
            "/grid": { "LineGrid3D": { "plane": [0.0, 0.0, 1.0, 0.0] } },
            "/spatial": { "SpatialInformation": { "target_frame": "world" } },
            "/table": {
                "TableColumn": { "editable": true },
                "TableLayout": { "column_order": ["x", "y"] },
            },
            "/time_axis": { "TimeAxis": { "view_range": {
                "start": { "CursorRelative": -10 },
                "end": "Infinite",
            } } },
            "/time_panel": { "TimePanelBlueprint": {
                "fps": 30.0,
                "playback_speed": 2.0,
                "time_selection": { "min": 1, "max": 5 },
            } },
            "/visible": { "VisibleTimeRanges": { "ranges": {
                "timeline": "frame",
                "range": { "start": { "Absolute": 3 }, "end": { "CursorRelative": 0 } },
            } } },
            "/visualizer": { "VisualizerInstruction": { "component_map": {
                "target": "Points3D:positions",
                "source_kind": 1,
                "source_component": "my_points",
                "selector": null,
            } } },
        });

        let mut blueprint = empty_blueprint();
        write(&mut blueprint, &json);

        assert_eq!(read(&blueprint), json);
    }

    /// Built-in archetypes are keyed by short name only while no two share one.
    #[test]
    fn builtin_archetype_short_names_are_unique() {
        let reflection = re_sdk_types::reflection::reflection();
        let mut short_names: Vec<_> = reflection
            .archetypes
            .keys()
            .map(|name| name.short_name())
            .collect();
        short_names.sort_unstable();
        let before = short_names.len();
        short_names.dedup();
        assert_eq!(short_names.len(), before, "duplicate archetype short names");
    }

    /// An archetype registered at runtime may share a built-in's short name. Both are then keyed
    /// by full name, and the short name alone is an error rather than a guess.
    #[test]
    fn archetypes_sharing_a_short_name_are_keyed_by_full_name() {
        let mut reflection = re_sdk_types::reflection::reflection().clone();
        let builtin = ArchetypeName::from("rerun.blueprint.archetypes.ViewBlueprint");
        let custom = ArchetypeName::from("rerun.ViewBlueprint");
        let copy = reflection.archetypes[&builtin].clone();
        reflection.archetypes.insert(custom, copy);

        let json = json!({
            "/a": { "rerun.blueprint.archetypes.ViewBlueprint": { "class_identifier": "3D" } },
            "/b": { "rerun.ViewBlueprint": { "class_identifier": "2D" } },
        });
        let mut blueprint = empty_blueprint();
        let chunks = chunks_from_json(
            &json,
            blueprint.storage_engine().store(),
            &TimePoint::default(),
            &reflection,
        )
        .unwrap();
        for chunk in chunks {
            blueprint.add_chunk(&Arc::new(chunk)).unwrap();
        }
        let query = LatestAtQuery::latest(TimelineName::from_static_str("blueprint"));
        let read_back =
            crate::json_from_store(blueprint.storage_engine().store(), &query, &reflection);
        assert_eq!(read_back, json);

        let error = chunks_from_json(
            &json!({ "/a": { "ViewBlueprint": {} } }),
            empty_blueprint().storage_engine().store(),
            &TimePoint::default(),
            &reflection,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            "/a: ambiguous archetype \"ViewBlueprint\"; use the full name of one of: rerun.ViewBlueprint, rerun.blueprint.archetypes.ViewBlueprint"
        );
    }

    #[test]
    fn what_the_json_leaves_out_is_cleared() {
        let mut blueprint = example_blueprint();
        let mut json = read(&blueprint);
        let (_, plot) = view_paths(&json);
        json.as_object_mut().unwrap().remove(&plot);

        write(&mut blueprint, &json);

        assert_eq!(read(&blueprint), json);
    }

    /// A `Vec3D` is itself an array, so one of them and a batch of them differ only in nesting.
    #[test]
    fn nesting_tells_one_value_from_a_batch() {
        let one = json!({ "/view/v/EyeControls3D": { "EyeControls3D": { "position": [3.0, 3.0, 2.0] } } });
        let batch = json!({ "/view/v/EyeControls3D": { "EyeControls3D": { "position": [[3.0, 3.0, 2.0]] } } });

        for json in [&one, &batch] {
            let mut blueprint = empty_blueprint();
            write(&mut blueprint, json);
            assert_eq!(read(&blueprint), one);
        }
    }

    #[test]
    fn special_forms_are_read_back() {
        let json = json!({ "/container/c": { "ContainerBlueprint": {
            "container_kind": "Grid",
            "active_tab": "/view/v",
        } }, "/viewport": { "ViewportBlueprint": {
            "root_container": "4a8c0000-0000-4000-8000-000000000001",
        } }, "/view/v/ViewContents/overrides/world/points": { "Points3D": {
            "colors": ["#ff0010", "#00ff0080"],
        } } });

        let mut blueprint = empty_blueprint();
        write(&mut blueprint, &json);

        assert_eq!(read(&blueprint), json);
    }

    #[test]
    fn mistakes_name_where_they_are_and_what_was_expected() {
        let error = |json: Value| {
            chunks_from_json(
                &json,
                empty_blueprint().storage_engine().store(),
                &TimePoint::default(),
                re_sdk_types::reflection::reflection(),
            )
            .unwrap_err()
            .to_string()
        };

        insta::assert_snapshot!(
            [
                error(json!([])),
                error(json!({ "/view/v": { "ViewBlueprnt": {} } })),
                error(json!({ "/view/v": { "ViewBlueprint": { "origin": "/" } } })),
                error(json!({ "/c": { "ContainerBlueprint": { "container_kind": "Diagonal" } } })),
                error(json!({ "/c": { "ContainerBlueprint": { "col_shares": [["x"]] } } })),
                error(json!({ "/t": { "TimeAxis": { "view_range": { "start": "Forever", "end": "Infinite" } } } })),
                error(json!({ "/t": { "TimeAxis": { "view_range": {
                    "start": "Infinite",
                    "end": "Infinite",
                    "middle": "Infinite",
                } } } })),
                error(
                    json!({ "/viewport": { "ViewportBlueprint": { "root_container": "nope" } } })
                ),
                error(json!({ "/view/v": { "ViewBlueprint": { "visible": "yes" } } })),
                error(json!({ "/c": { "ContainerBlueprint": { "container_kind": 300 } } })),
            ]
            .join("\n")
        );
    }

    /// The paths of the example's two views: the one with contents, then the other.
    fn view_paths(json: &Value) -> (String, String) {
        let mut views = json
            .as_object()
            .unwrap()
            .iter()
            .filter(|(path, entity)| {
                path.starts_with("/view/") && entity.get("ViewBlueprint").is_some()
            })
            .map(|(path, entity)| {
                (
                    entity["ViewBlueprint"]["class_identifier"].clone(),
                    path.clone(),
                )
            });
        let scene = views.clone().find(|(class, _)| class == "3D").unwrap().1;
        let plot = views.find(|(class, _)| class == "TimeSeries").unwrap().1;
        (scene, plot)
    }
}
