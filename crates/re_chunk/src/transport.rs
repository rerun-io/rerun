use std::collections::BTreeMap;

use arrow2::{
    array::{Array as ArrowArray, PrimitiveArray as ArrowPrimitiveArray},
    chunk::Chunk as ArrowChunk,
    datatypes::{
        DataType as ArrowDatatype, Field as ArrowField, Metadata as ArrowMetadata,
        Schema as ArrowSchema, TimeUnit as ArrowTimeUnit,
    },
};

use re_log_types::{EntityPath, RowId, TimeInt, Timeline};
use re_types_core::Loggable as _;

use crate::{Chunk, ChunkError, ChunkResult, ChunkTimeline};

// ---

/// A [`Chunk`] that is ready for transport. Obtained by calling [`Chunk::to_transport`].
///
/// Implemented as an Arrow dataframe: a schema and a batch.
///
/// Use the `Display` implementation to dump the chunk as a nicely formatted table.
///
/// This has a stable ABI! The entire point of this type is to allow users to send raw arrow data
/// into Rerun.
/// This means we have to be very careful when checking the validity of the data: slipping corrupt
/// data into the store could silently break all the index search logic (e.g. think of a chunk
/// claiming to be sorted while it is in fact not).
//
// TODO(#4184): Provide APIs in all SDKs to log these all at once (temporal batches).
#[derive(Debug)]
pub struct TransportChunk {
    /// The schema of the dataframe, and all chunk-level and field-level metadata.
    ///
    /// Take a look at the `TransportChunk::CHUNK_METADATA_*` and `TransportChunk::FIELD_METADATA_*
    /// constants for more information about available metadata.
    pub schema: ArrowSchema,

    /// All the control, time and component data.
    pub data: ArrowChunk<Box<dyn ArrowArray>>,
}

impl std::fmt::Display for TransportChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chunk = Chunk::from_transport(self).map_err(|err| {
            re_log::error_once!("couldn't display TransportChunk: {err}");
            std::fmt::Error
        })?;

        f.write_fmt(format_args!("{chunk}"))
    }
}

impl TransportChunk {
    /// The key used to identify a Rerun [`EntityPath`] in chunk-level [`ArrowSchema`] metadata.
    pub const CHUNK_METADATA_KEY_ENTITY_PATH: &'static str = "rerun.chunk.entity_path";

    /// The key used to identify whether the chunk is sorted in chunk-level [`ArrowSchema`] metadata.
    pub const CHUNK_METADATA_KEY_IS_SORTED: &'static str = "rerun.chunk.is_sorted";

    /// The key used to identify the kind of a Rerun column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_KEY_KIND: &'static str = "rerun.field.kind";

    /// The value used to identify a Rerun time column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_TIME: &'static str = "time";

    /// The value used to identify a Rerun control column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_CONTROL: &'static str = "control";

    /// The value used to identify a Rerun data column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_DATA: &'static str = "data";

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun time column.
    #[inline]
    pub fn metadata_time_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_TIME.to_owned(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun control column.
    #[inline]
    pub fn metadata_control_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_CONTROL.to_owned(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun data column.
    #[inline]
    pub fn metadata_data_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_DATA.to_owned(),
            ), //
        ]
        .into()
    }
}

impl TransportChunk {
    #[inline]
    pub fn entity_path(&self) -> ChunkResult<EntityPath> {
        match self
            .schema
            .metadata
            .get(Self::CHUNK_METADATA_KEY_ENTITY_PATH)
        {
            Some(entity_path) => Ok(EntityPath::parse_forgiving(entity_path)),
            None => Err(crate::ChunkError::Malformed {
                reason: format!(
                    "entity path missing from metadata ({:?})",
                    self.schema.metadata
                ),
            }),
        }
    }

    /// Looks in the chunk metadata for the `IS_SORTED` marker.
    ///
    /// It is possible that a chunk is sorted but didn't set that marker.
    /// This is fine, if wasteful.
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.schema
            .metadata
            .get(Self::CHUNK_METADATA_KEY_IS_SORTED)
            .is_some()
    }

    /// Iterates all columns of the specified `kind`.
    ///
    /// See:
    /// * [`Self::FIELD_METADATA_VALUE_KIND_TIME`]
    /// * [`Self::FIELD_METADATA_VALUE_KIND_CONTROL`]
    /// * [`Self::FIELD_METADATA_VALUE_KIND_DATA`]
    #[inline]
    pub fn columns<'a>(
        &'a self,
        kind: &'a str,
    ) -> impl Iterator<Item = (&ArrowField, &'a Box<dyn ArrowArray>)> + 'a {
        self.schema
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                let actual_kind = field.metadata.get(Self::FIELD_METADATA_KEY_KIND);
                (actual_kind.map(|s| s.as_str()) == Some(kind))
                    .then(|| self.data.columns().get(i).map(|column| (field, column)))
                    .flatten()
            })
    }

    /// Iterates all control columns present in this chunk.
    #[inline]
    pub fn controls(&self) -> impl Iterator<Item = (&ArrowField, &Box<dyn ArrowArray>)> {
        self.columns(Self::FIELD_METADATA_VALUE_KIND_CONTROL)
    }

    /// Iterates all data columns present in this chunk.
    #[inline]
    pub fn components(&self) -> impl Iterator<Item = (&ArrowField, &Box<dyn ArrowArray>)> {
        self.columns(Self::FIELD_METADATA_VALUE_KIND_DATA)
    }

    /// Iterates all timeline columns present in this chunk.
    #[inline]
    pub fn timelines(&self) -> impl Iterator<Item = (&ArrowField, &Box<dyn ArrowArray>)> {
        self.columns(Self::FIELD_METADATA_VALUE_KIND_TIME)
    }

    /// How many columns total? Includes control, time, and component columns.
    #[inline]
    pub fn num_columns(&self) -> usize {
        self.data.columns().len()
    }

    #[inline]
    pub fn num_controls(&self) -> usize {
        self.controls().count()
    }

    #[inline]
    pub fn num_timelines(&self) -> usize {
        self.timelines().count()
    }

    #[inline]
    pub fn num_components(&self) -> usize {
        self.components().count()
    }

    #[inline]
    pub fn num_rows(&self) -> usize {
        self.data.columns().first().map_or(0, |column| column.len())
    }
}

impl Chunk {
    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    pub fn to_transport(&self) -> ChunkResult<TransportChunk> {
        self.sanity_check()?;

        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            self.num_columns(),
            self.num_rows()
        ));

        let Self {
            entity_path,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let mut schema = ArrowSchema::default();
        let mut columns = Vec::with_capacity(1 /* row_ids */ + timelines.len() + components.len());

        // Chunk-level metadata
        {
            re_tracing::profile_scope!("metadata");

            schema.metadata.insert(
                TransportChunk::CHUNK_METADATA_KEY_ENTITY_PATH.to_owned(),
                entity_path.to_string(),
            );

            if *is_sorted {
                schema.metadata.insert(
                    TransportChunk::CHUNK_METADATA_KEY_IS_SORTED.to_owned(),
                    String::new(),
                );
            }
        }

        // Row IDs
        {
            re_tracing::profile_scope!("row ids");

            let row_ids = RowId::to_arrow(row_ids)?;
            schema.fields.push(
                ArrowField::new(
                    RowId::name().to_string(),
                    row_ids.data_type().clone(),
                    false,
                )
                .with_metadata(TransportChunk::metadata_control_column()),
            );
            columns.push(row_ids);
        }

        // Timelines
        {
            re_tracing::profile_scope!("timelines");

            for (timeline, info) in timelines {
                let ChunkTimeline {
                    times,
                    is_sorted: _,
                    time_range: _,
                } = info;

                let times = {
                    let values = times.iter().map(|time| time.as_i64()).collect();
                    ArrowPrimitiveArray::new(
                        arrow2::types::PrimitiveType::Int64.into(),
                        values,
                        None,
                    )
                    .to(timeline.datatype())
                };

                schema.fields.push(
                    ArrowField::new(
                        timeline.name().to_string(),
                        times.data_type().clone(),
                        false,
                    )
                    .with_metadata(TransportChunk::metadata_time_column()),
                );
                columns.push(Box::new(times));
            }
        }

        // Components
        {
            re_tracing::profile_scope!("components");

            for (component_name, data) in components {
                schema.fields.push(
                    ArrowField::new(component_name.to_string(), data.data_type().clone(), true)
                        .with_metadata(TransportChunk::metadata_data_column()),
                );
                columns.push(data.clone() /* refcounted (dyn Clone) */);
            }
        }

        Ok(TransportChunk {
            schema,
            data: ArrowChunk::new(columns),
        })
    }

    pub fn from_transport(chunk: &TransportChunk) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            chunk.num_columns(),
            chunk.num_rows()
        ));

        // Entity path (metadata only)
        let entity_path = {
            re_tracing::profile_scope!("entity path");
            chunk.entity_path()?
        };

        // Row IDs
        let row_ids = {
            re_tracing::profile_scope!("row ids");

            let Some(column) = chunk.controls().find_map(|(field, column)| {
                (field.name == RowId::name().as_str()).then_some(column)
            }) else {
                return Err(ChunkError::Malformed {
                    reason: format!("missing row_id column ({:?})", chunk.schema),
                });
            };

            RowId::from_arrow(&**column).map_err(|err| ChunkError::Malformed {
                reason: format!("row_id column is not deserializable: {err}"),
            })?
        };

        // Timelines
        let timelines = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = BTreeMap::default();

            for (field, column) in chunk.timelines() {
                // See also [`Timeline::datatype`]
                let timeline = match column.data_type().to_logical_type() {
                    ArrowDatatype::Int64 => Timeline::new_sequence(field.name.as_str()),
                    ArrowDatatype::Timestamp(ArrowTimeUnit::Nanosecond, None) => {
                        Timeline::new_temporal(field.name.as_str())
                    }
                    _ => {
                        return Err(ChunkError::Malformed {
                            reason: format!(
                                "time column '{}' is not deserializable ({:?})",
                                field.name,
                                column.data_type()
                            ),
                        });
                    }
                };

                let times = column
                    .as_any()
                    .downcast_ref::<ArrowPrimitiveArray<i64>>()
                    .ok_or_else(|| ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' is not deserializable ({:?})",
                            field.name,
                            column.data_type()
                        ),
                    })?;

                if times.validity().is_some() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' must be dense ({:?})",
                            field.name,
                            column.data_type()
                        ),
                    });
                }

                if timelines
                    .insert(
                        timeline,
                        times
                            .values_iter()
                            .copied()
                            .map(TimeInt::new_temporal)
                            .collect(),
                    )
                    .is_some()
                {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' was specified more than once",
                            field.name,
                        ),
                    });
                }
            }

            timelines
        };

        // Components
        let components = {
            let mut components = BTreeMap::default();

            for (field, column) in chunk.components() {
                if !matches!(column.data_type(), ArrowDatatype::List(_)) {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "component column '{}' is not deserializable ({:?})",
                            field.name,
                            column.data_type()
                        ),
                    });
                }

                if components
                    .insert(
                        field.name.clone().into(),
                        column.clone(), /* refcount */
                    )
                    .is_some()
                {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "component column '{}' was specified more than once",
                            field.name,
                        ),
                    });
                }
            }

            components
        };

        Self::new(
            entity_path,
            chunk.is_sorted().then_some(true),
            row_ids,
            timelines,
            components,
        )
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::{
        example_components::{MyColor, MyPoint},
        TimeInt, Timeline,
    };

    use crate::arrays_to_list_array;

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let entity_path = EntityPath::parse_forgiving("a/b/c");

        let timeline1 = Timeline::new_temporal("log_time");
        let timelines1 = std::iter::once((
            timeline1,
            [42, 43, 44, 45].map(TimeInt::new_temporal).to_vec(),
        ))
        .collect();

        let timelines2 = BTreeMap::default(); // static

        let points1 = MyPoint::to_arrow([
            MyPoint::new(1.0, 2.0),
            MyPoint::new(3.0, 4.0),
            MyPoint::new(5.0, 6.0),
        ])?;
        let points2 = None;
        let points3 = MyPoint::to_arrow([MyPoint::new(10.0, 20.0)])?;
        let points4 = MyPoint::to_arrow([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let colors1 = MyColor::to_arrow([
            MyColor::from_rgb(1, 2, 3),
            MyColor::from_rgb(4, 5, 6),
            MyColor::from_rgb(7, 8, 9),
        ])?;
        let colors2 = MyColor::to_arrow([MyColor::from_rgb(10, 20, 30)])?;
        let colors3 = None;
        let colors4 = None;

        let components = [
            (MyPoint::name(), {
                let list_array = arrays_to_list_array(&[
                    Some(&*points1),
                    points2,
                    Some(&*points3),
                    Some(&*points4),
                ])
                .unwrap();
                assert_eq!(4, list_array.len());
                list_array
            }),
            (MyPoint::name(), {
                let list_array =
                    arrays_to_list_array(&[Some(&*colors1), Some(&*colors2), colors3, colors4])
                        .unwrap();
                assert_eq!(4, list_array.len());
                list_array
            }),
        ];

        let row_ids = vec![RowId::new(), RowId::new(), RowId::new(), RowId::new()];

        for timelines in [timelines1, timelines2] {
            let chunk_original = Chunk::new(
                entity_path.clone(),
                None,
                row_ids.clone(),
                timelines.clone(),
                components.clone().into_iter().collect(),
            )?;
            let mut chunk_before = chunk_original.clone();

            for _ in 0..3 {
                let chunk_in_transport = chunk_before.to_transport()?;
                let chunk_after = Chunk::from_transport(&chunk_in_transport)?;

                assert_eq!(
                    chunk_in_transport.entity_path()?,
                    *chunk_original.entity_path()
                );
                assert_eq!(
                    chunk_in_transport.entity_path()?,
                    *chunk_after.entity_path()
                );
                assert_eq!(
                    chunk_in_transport.num_columns(),
                    chunk_original.num_columns()
                );
                assert_eq!(chunk_in_transport.num_columns(), chunk_after.num_columns());
                assert_eq!(chunk_in_transport.num_rows(), chunk_original.num_rows());
                assert_eq!(chunk_in_transport.num_rows(), chunk_after.num_rows());

                assert_eq!(
                    chunk_in_transport.num_controls(),
                    chunk_original.num_controls()
                );
                assert_eq!(
                    chunk_in_transport.num_controls(),
                    chunk_after.num_controls()
                );
                assert_eq!(
                    chunk_in_transport.num_timelines(),
                    chunk_original.num_timelines()
                );
                assert_eq!(
                    chunk_in_transport.num_timelines(),
                    chunk_after.num_timelines()
                );
                assert_eq!(
                    chunk_in_transport.num_components(),
                    chunk_original.num_components()
                );
                assert_eq!(
                    chunk_in_transport.num_components(),
                    chunk_after.num_components()
                );

                eprintln!("{chunk_before}");
                eprintln!("{chunk_in_transport:#?}");
                eprintln!("{chunk_after}");

                assert_eq!(chunk_before, chunk_after);

                chunk_before = chunk_after;
            }
        }

        Ok(())
    }
}
