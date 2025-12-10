use std::collections::{BTreeMap, HashMap};

use arrow::array::{BooleanArray, FixedSizeBinaryArray, StringArray, UInt64Array};
use arrow::buffer::NullBuffer;
use arrow::datatypes::Field;
use itertools::Itertools as _;
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{ArchetypeName, ChunkError, ChunkId, ComponentIdentifier, ComponentType, Timeline};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, TimeType};
use re_types_core::ComponentDescriptor;

use crate::{CodecResult, Decodable as _, StreamFooterEntry, ToApplication as _};

// TODO: probably should have more drastic checks for chunk_num_rows

// ---

/// This is the payload that is carried in messages of type `::End` in RRD streams.
///
/// It keeps track of various useful information about the associated recording.
///
/// During normal operations, there can only be a single `::End` message in an RRD stream, and
/// therefore a single `RrdFooter`.
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one `::End` message is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
///
/// This is an application-level type, the associated transport-level type can be found
/// over at [`re_protos::log_msg::v1alpha1::RrdFooter`].
#[derive(Default, Debug)]
pub struct RrdFooter {
    /// All the [`RrdManifest`]s that were found in this RRD footer.
    ///
    /// Each [`RrdManifest`] corresponds to one, and exactly one, RRD stream (i.e. recording).
    ///
    /// The order is unspecified.
    pub manifests: HashMap<StoreId, RrdManifest>,
}

// TODO: update the snippet
//
/// The payload found in [`RrdFooter`]s.
///
/// Each `RrdManifest` corresponds to one, and exactly one, RRD stream (i.e. recording).
/// This restriction exists to make working with multiple RRD streams much simpler: due to the way
/// the Rerun data model works, filtering rows of data from a manifest can have hard-to-predict
/// second order effects on the schema of the stream as a whole.
/// By keeping manifests for different recordings separate, we remove the need to filter per
/// recording ID, greatly simplifying the process.
///
/// This is an application-level type, the associated transport-level type can be found
/// over at [`re_protos::log_msg::v1alpha1::RrdManifest`].
///
/// ## What's in the box?
///
/// Each row in this manifest describes a unique chunk (ID, offset, size, timeline & component stats, etc).
/// This can be used to compute relevancy queries (latest-at, range, dataframe), without needing to load
/// any of the actual data in memory.
///
/// You can think of an RRD Manifest as a dataframe of the time panel, effectively.
///
/// The best way to understand what an RRD manifest does or doesn't do is to look at snapshots for
/// a simple recording:
/// ```text,ignore
/// ┌─────────────────────────────────────────┬──────────────────────────────────┬──────────────────────────────────┐
/// │ chunk_entity_path                       ┆ /my_entity                       ┆ /my_entity                       │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_id                                ┆ 00000000000000010000000000000001 ┆ 00000000000000010000000000000002 │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_is_static                         ┆ false                            ┆ true                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_byte_offset                       ┆ 104                              ┆ 1552                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_byte_size                         ┆ 1432                             ┆ 947                              │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:colors:has_static_data ┆ false                            ┆ false                            │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:labels:has_static_data ┆ false                            ┆ true                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:points:has_static_data ┆ false                            ┆ false                            │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:start                          ┆ 10                               ┆ null                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:end                            ┆ 40                               ┆ null                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:start  ┆ 20                               ┆ null                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:end    ┆ 30                               ┆ null                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:start  ┆ 10                               ┆ null                             │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:end    ┆ 40                               ┆ null                             │
/// └─────────────────────────────────────────┴──────────────────────────────────┴──────────────────────────────────┘
/// ```
///
/// Note that `:start` & `:end` columns are always implicitly _inclusive_. The `_inclusive` suffix has been
/// removed to reduce noise.
///
/// ## A note on filtering
///
/// Always be on your toes when filtering rows out of an RRD manifest. Due to how the Rerun data
/// model works, removing rows (and therefore chunks) from a recording can also affect the number
/// of columns in that recording (because e.g. all the data for a specific entity path is gone),
/// which in turn will affect the Sorbet schema of the recording too.
///
/// Filtering RRD manifests is very non trivial and should only be performed with great care.
#[derive(Clone, Debug)]
pub struct RrdManifest {
    /// The recording ID that was used to identify the original recording.
    ///
    /// This is extracted from the `SetStoreInfo` message of the associated RRD stream.
    pub store_id: StoreId,

    /// The Sorbet schema of the recording, following the usual merging and sorting rules.
    ///
    /// ⚠️ This is the Sorbet schema of the recording being indexed by this manifest, *not* the
    /// schema of [`Self::data`].
    pub sorbet_schema: arrow::datatypes::Schema,

    /// The SHA256 hash of the Sorbet schema of the associated RRD stream.
    ///
    /// This is always computed by sorting the fields of the schema by name first.
    /// See [`RrdManifest::compute_sorbet_schema_sha256]`.
    pub sorbet_schema_sha256: [u8; 32],

    /// The actual manifest data, which catalogs every chunk in this recording.
    ///
    /// Each row in this dataframe describes a unique chunk (ID, offset, size, timeline & component stats, etc).
    /// This can be used to compute relevancy queries (latest-at, range, dataframe), without needing to load
    /// any of the actual data in memory.
    ///
    /// Note in particular that there isn't any recording ID column in here, since an [`RrdManifest`]
    /// is always scoped to a single recording: the one specified in [`RrdManifest::store_id`].
    //
    // TODO(cmc): should we slap a sorbet:version on this? that probably should be part of the sorbet ABI as
    // much as anything else?
    pub data: arrow::array::RecordBatch,
}

pub type NativeStaticMap = IntMap<EntityPath, IntMap<ComponentIdentifier, ChunkId>>;

#[derive(Debug, Clone, Copy)]
pub struct NativeTemporalMapEntry {
    pub time_range: AbsoluteTimeRange,

    // TODO: aka num_events or something
    pub num_rows: u64,
}

pub type NativeTemporalMap = IntMap<
    EntityPath,
    IntMap<Timeline, IntMap<ComponentIdentifier, BTreeMap<ChunkId, NativeTemporalMapEntry>>>,
>;

impl RrdManifest {
    // TODO
    pub fn from_rrd_bytes(rrd_bytes: &[u8]) -> CodecResult<Option<Self>> {
        let stream_footer = match crate::StreamFooter::from_rrd_bytes(rrd_bytes) {
            Ok(footer) => footer,

            // That was in fact _not_ a footer.
            Err(crate::CodecError::FrameDecoding(_)) => return Ok(None),

            err @ Err(_) => err?,
        };

        if stream_footer.entries.len() != 1 {
            re_log::warn!(
                num_manifests = stream_footer.entries.len(),
                "detected recording with unsupported number of manifests, falling back to slow path",
            );
            return Ok(None);
        }

        let StreamFooterEntry {
            rrd_footer_byte_span_from_start_excluding_header,
            crc_excluding_header,
        } = stream_footer.entries[0];

        let rrd_footer_byte_span = rrd_footer_byte_span_from_start_excluding_header;
        if rrd_footer_byte_span.start == 0 || rrd_footer_byte_span.len == 0 {
            re_log::warn!(
                num_manifests = stream_footer.entries.len(),
                "detected recording with corrupt footer, falling back to slow path",
            );
            return Ok(None);
        }

        let rrd_footer_bytes =
            &rrd_bytes[rrd_footer_byte_span.try_cast::<usize>().unwrap().range()];

        let crc = crate::StreamFooter::compute_crc(rrd_footer_bytes);
        if crc != crc_excluding_header {
            return Err(crate::CodecError::CrcMismatch {
                expected: crc_excluding_header,
                got: crc,
            });
        }

        let rrd_footer = re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes)?;
        rrd_footer
            .manifests
            .iter()
            .find(|manifest| {
                manifest
                    .store_id
                    .as_ref()
                    .map(|id| id.kind() == re_protos::common::v1alpha1::StoreKind::Recording)
                    .unwrap_or(false)
            })
            .map(|manifest| manifest.to_application(()))
            .transpose()
    }

    // TODO
    pub fn to_native_static(&self) -> CodecResult<NativeStaticMap> {
        use re_arrow_util::ArrowArrayDowncastRef as _;

        let mut per_entity: NativeStaticMap = IntMap::default();

        let chunk_ids = self.col_chunk_id()?;
        let chunk_entity_paths = self.col_chunk_entity_path()?;
        let chunk_is_static = self.col_chunk_is_static()?;

        let has_static_component_data =
            itertools::izip!(self.data.schema_ref().fields().iter(), self.data.columns(),)
                .filter(|(f, _c)| f.name().ends_with(":has_static_data"))
                .map(|(f, c)| {
                    (
                        f,
                        c.downcast_array_ref::<arrow::array::BooleanArray>()
                            .unwrap(), // TODO
                    )
                })
                .collect_vec();

        for (i, (chunk_id, is_static, entity_path)) in
            itertools::izip!(chunk_ids, chunk_is_static, chunk_entity_paths).enumerate()
        {
            if !is_static {
                continue;
            }

            for (f, has_static_component_data) in &has_static_component_data {
                let has_static_component_data = has_static_component_data.value(i);
                if !has_static_component_data {
                    continue;
                }

                let component = f.metadata().get("rerun:component").unwrap();
                let component = ComponentIdentifier::new(component);

                let per_component = per_entity.entry(entity_path.clone()).or_default();

                // TODO: well that's a problem, what are supposed to be the winning semantics here again?
                per_component
                    .entry(component)
                    .and_modify(|id| *id = chunk_id)
                    .or_insert(chunk_id);
            }
        }

        Ok(per_entity)
    }

    /// Convert it to a more convenient structure.
    pub fn to_native_temporal(&self) -> CodecResult<NativeTemporalMap> {
        re_tracing::profile_function!();

        use arrow::array::ArrayRef;
        use re_arrow_util::ArrowArrayDowncastRef as _;

        fn downcast_index_as_int64_slice(array: &ArrayRef) -> Option<&[i64]> {
            let (_typ, values) = TimeType::from_arrow_array(array).ok()?;
            Some(values)
        }

        let fields = self.data.schema_ref().fields();
        let columns = self.data.columns();
        let indexes = fields
            .iter()
            .filter_map(|f| {
                f.metadata()
                    .get("rerun:index")
                    .and_then(|index| f.metadata().get("rerun:component").map(|c| (index, c)))
            })
            .unique()
            .collect_vec();

        let mut per_entity: NativeTemporalMap = Default::default();

        let chunk_ids = self.col_chunk_id()?;
        let chunk_entity_paths = self.col_chunk_entity_path()?;
        let chunk_is_static = self.col_chunk_is_static()?;

        for (i, (chunk_id, is_static, entity_path)) in
            itertools::izip!(chunk_ids, chunk_is_static, chunk_entity_paths).enumerate()
        {
            if is_static {
                continue;
            }

            for (index, component) in &indexes {
                let index = index.as_str();
                if index == "rerun:static" {
                    continue;
                }

                pub fn get_index_name(field: &arrow::datatypes::Field) -> Option<&str> {
                    field.metadata().get("rerun:index").map(|s| s.as_str())
                }

                pub fn is_specific_index(
                    field: &arrow::datatypes::Field,
                    index_name: &str,
                ) -> bool {
                    get_index_name(field) == Some(index_name)
                }

                pub fn is_index_start(field: &arrow::datatypes::Field) -> bool {
                    field.name().ends_with(":start")
                }

                pub fn is_index_end(field: &arrow::datatypes::Field) -> bool {
                    field.name().ends_with(":end")
                }

                pub fn is_index_num_rows(field: &arrow::datatypes::Field) -> bool {
                    field.name().ends_with(":num_rows")
                }

                // TODO: obviously all of this stuff should be done only once, not every iteration 🫠

                let col_start = itertools::izip!(fields, columns).find(|(f, _col)| {
                    is_specific_index(f, index)
                        && is_index_start(f)
                        && f.metadata().get("rerun:component") == Some(component)
                });
                let col_end = itertools::izip!(fields, columns).find(|(f, _col)| {
                    is_specific_index(f, index)
                        && is_index_end(f)
                        && f.metadata().get("rerun:component") == Some(component)
                });
                let col_num_rows = itertools::izip!(fields, columns).find(|(f, _col)| {
                    is_specific_index(f, index)
                        && is_index_num_rows(f)
                        && f.metadata().get("rerun:component") == Some(component)
                });

                let (Some((_, col_start)), Some((_, col_end))) = (col_start, col_end) else {
                    unreachable!();
                };

                let col_start_raw = downcast_index_as_int64_slice(col_start).unwrap();
                let col_end_raw = downcast_index_as_int64_slice(col_end).unwrap();
                // TODO: optional because BW, but do we care? maybe, maybe not
                let col_num_rows = col_num_rows.as_ref().map(|(f, col_num_rows)| {
                    let values: &[u64] = col_num_rows
                        .downcast_array_ref::<UInt64Array>()
                        .unwrap()
                        .values();
                    values
                });

                // So we don't have to pay the virtual call cost for every `is_valid()` call.
                let col_start_nulls = col_start
                    .nulls()
                    .cloned()
                    .unwrap_or_else(|| NullBuffer::new_valid(col_start.len()));
                let col_end_nulls = col_end
                    .nulls()
                    .cloned()
                    .unwrap_or_else(|| NullBuffer::new_valid(col_end.len()));

                if !col_start_nulls.is_valid(i) || !col_end_nulls.is_valid(i) {
                    continue;
                }

                let component = ComponentIdentifier::new(component);
                let timeline = match col_start.data_type() {
                    arrow::datatypes::DataType::Int64 => Timeline::new_sequence(index),
                    arrow::datatypes::DataType::Timestamp(_, _) => Timeline::new_timestamp(index),
                    arrow::datatypes::DataType::Duration(_) => Timeline::new_duration(index),
                    _ => unreachable!(), // TODO
                };

                let per_timeline = per_entity.entry(entity_path.clone()).or_default();
                let per_component = per_timeline.entry(timeline).or_default();
                let per_chunk = per_component.entry(component).or_default();

                let start = col_start_raw[i];
                let end = col_end_raw[i];
                let entry = NativeTemporalMapEntry {
                    time_range: AbsoluteTimeRange::new(start, end),
                    // TODO: i mean if BW then this sucks anyway, cause now you cannot
                    // differentiate 0 from not present...
                    num_rows: col_num_rows.as_ref().unwrap()[i],
                };

                per_chunk
                    .entry(chunk_id)
                    .and_modify(|tr| *tr = entry)
                    .or_insert(entry);
            }
        }

        Ok(per_entity)
    }
}

// Schema fields are stored as Vecs, but we don't want their order to matter when performing comparisons.
impl PartialEq for RrdManifest {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            store_id,
            sorbet_schema,
            sorbet_schema_sha256,
            data,
        } = self;

        *store_id == other.store_id
            && *data == other.data
            && *sorbet_schema_sha256 == other.sorbet_schema_sha256
            && sorbet_schema.metadata() == other.sorbet_schema.metadata()
            && sorbet_schema.fields.len() == other.sorbet_schema.fields.len()
            && {
                let sorted_fields = sorbet_schema.fields.iter().sorted_by_key(|f| f.name());
                let other_sorted_fields = other
                    .sorbet_schema
                    .fields
                    .iter()
                    .sorted_by_key(|f| f.name());
                itertools::izip!(sorted_fields, other_sorted_fields).all(|(f1, f2)| f1 == f2)
            }
    }
}

// Helpers
impl RrdManifest {
    pub fn compute_sorbet_schema_sha256(
        schema: &arrow::datatypes::Schema,
    ) -> Result<[u8; 32], arrow::error::ArrowError> {
        let schema = {
            // Sort and remove top-level metadata before hashing.
            let mut fields = schema.fields().to_vec();
            fields.sort();
            arrow::datatypes::Schema::new_with_metadata(fields, Default::default()) // no metadata!
        };

        let partition_schema_ipc = {
            let mut schema_ipc = Vec::new();
            arrow::ipc::writer::StreamWriter::try_new(&mut schema_ipc, &schema)?;
            schema_ipc
        };

        use sha2::Digest as _;
        let mut hash = [0u8; 32];
        let mut hasher = sha2::Sha256::new();
        hasher.update(&partition_schema_ipc);
        hasher.finalize_into(sha2::digest::generic_array::GenericArray::from_mut_slice(
            &mut hash,
        ));

        Ok(hash)
    }

    /// Computes the appropriate column name for the provided parts.
    ///
    /// The name is guaranteed to be sanitized and safe to use in all environments where Rerun data
    /// can generally be found (Lance, external dataframe libraries, etc).
    ///
    /// If caller doesn't provide any part (i.e. all are `None`), an empty string is returned.
    pub fn compute_column_name(
        entity_path: Option<&EntityPath>,
        strip_entity_prefix: Option<&str>,
        component_desc: Option<&ComponentDescriptor>,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> String {
        use re_types_core::reflection::ComponentDescriptorExt as _;
        let full_name = [
            prefix.map(ToOwned::to_owned),
            entity_path.map(|p| {
                let path = p.to_string();
                // Optionally strip the entity prefix if provided
                let path = strip_entity_prefix
                    .and_then(|prefix| path.strip_prefix(prefix))
                    .unwrap_or(&path);
                // Always strip trailing slashes (if present)
                path.strip_suffix("/").unwrap_or(path).to_owned()
            }),
            component_desc
                .and_then(|descr| descr.archetype)
                .map(|archetype| archetype.short_name().to_owned()),
            component_desc.map(|descr| descr.archetype_field_name().to_owned()),
            suffix.map(ToOwned::to_owned),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(":");

        // All of the following characters have proven to be problematic in one or more
        // environments. Replace them with `_`, unconditionally.
        let sanitized = full_name.replace([',', ' ', '-', '.', '\\'], "_");

        // Remove leading underscore if present
        sanitized.trim_start_matches('_').to_owned()
    }
}

// Sanity checks
impl RrdManifest {
    /// Checks the manifest for any traces of corruption.
    ///
    /// This is cheap to compute and is automatically performed when converting an [`RrdManifest`]
    /// from its transport-level to its application-level representation (and vice-versa).
    ///
    /// See [`Self::sanity_check_heavy`] for a more costly version that is not suitable to use in
    /// production, but can be useful in e.g. tests.
    pub fn sanity_check_cheap(&self) -> CodecResult<()> {
        self.check_global_columns_are_correct()?;
        self.check_index_columns_are_correct()?;
        self.check_manifest_schema_matches_sorbet_schema()?;
        Ok(())
    }

    /// Checks the manifest for any traces of corruption.
    ///
    /// This is quite costly and therefore should not be used on the happy production path.
    /// Prefer [`Self::sanity_check_cheap`] for that instead.
    pub fn sanity_check_heavy(&self) -> CodecResult<()> {
        self.check_sorbet_schema_sha256_is_correct()?;
        Ok(())
    }

    /// Cheap.
    fn check_global_columns_are_correct(&self) -> CodecResult<()> {
        _ = self.col_chunk_id()?;
        _ = self.col_chunk_is_static()?;
        _ = self.col_chunk_num_rows()?;
        _ = self.col_chunk_entity_path()?;
        _ = self.col_chunk_byte_offset()?;
        _ = self.col_chunk_byte_size()?;
        Ok(())
    }

    /// Cheap.
    fn check_index_columns_are_correct(&self) -> CodecResult<()> {
        {
            // All columns either end in :has_static_data or :start or :end (or are global).
            for field in self.data.schema().fields() {
                if let Some((_, suffix)) = field.name().rsplit_once(':') {
                    match suffix {
                        "start" | "end" => {
                            // Checked in depth below
                        }

                        "has_static_data" => {
                            if field.data_type() != Self::field_chunk_is_static().data_type() {
                                return Err(crate::CodecError::from(ChunkError::Malformed {
                                    reason: format!(
                                        "field '{}' should be {} but is actually {}",
                                        field.name(),
                                        Self::field_chunk_is_static().data_type(),
                                        field.data_type(),
                                    ),
                                }));
                            }
                        }

                        "num_rows" => {
                            if field.data_type() != Self::field_chunk_num_rows().data_type() {
                                return Err(crate::CodecError::from(ChunkError::Malformed {
                                    reason: format!(
                                        "field '{}' should be {} but is actually {}",
                                        field.name(),
                                        Self::field_chunk_num_rows().data_type(),
                                        field.data_type(),
                                    ),
                                }));
                            }
                        }

                        suffix => {
                            return Err(crate::CodecError::from(ChunkError::Malformed {
                                reason: format!(
                                    "field '{}' has invalid suffix '{suffix}'",
                                    field.name(),
                                ),
                            }));
                        }
                    }
                } else {
                    // Global column
                    match field.name().as_str() {
                        Self::FIELD_CHUNK_ID
                        | Self::FIELD_CHUNK_IS_STATIC
                        | Self::FIELD_CHUNK_NUM_ROWS
                        | Self::FIELD_CHUNK_BYTE_SIZE
                        | Self::FIELD_CHUNK_BYTE_OFFSET
                        | Self::FIELD_CHUNK_ENTITY_PATH => {}

                        name => {
                            return Err(crate::CodecError::from(ChunkError::Malformed {
                                reason: format!(
                                    "unexpected field '{name}' should not be present in an RRD manifest",
                                ),
                            }));
                        }
                    }
                }
            }
        }

        {
            // All `:start` columns should have a matching `:end`.
            // All `:end` columns should have a matching `:start`.
            for field in self.data.schema().fields() {
                if let Some((prefix, suffix)) = field.name().rsplit_once(':') {
                    let counterpart = match suffix {
                        "start" => "end",
                        "end" => "start",
                        _ => continue,
                    };

                    let field_counterpart = self
                        .data
                        .schema_ref()
                        .field_with_name(&format!("{prefix}:{counterpart}"))
                        .map_err(|_err| {
                            crate::CodecError::from(ChunkError::Malformed {
                                reason: format!(
                                    "field '{}' does not have matching `:{counterpart}` field",
                                    field.name()
                                ),
                            })
                        })?;

                    match field.data_type() {
                        arrow::datatypes::DataType::Int64
                        | arrow::datatypes::DataType::Timestamp(_, _)
                        | arrow::datatypes::DataType::Duration(_) => {}

                        datatype => {
                            return Err(crate::CodecError::from(ChunkError::Malformed {
                                reason: format!(
                                    "field '{}' is {datatype} which is not a supported index datatype",
                                    field.name(),
                                ),
                            }));
                        }
                    }

                    if field.data_type() != field_counterpart.data_type() {
                        return Err(crate::CodecError::from(ChunkError::Malformed {
                            reason: format!(
                                "field '{}' is {} but field '{}' is {}",
                                field.name(),
                                field.data_type(),
                                field_counterpart.name(),
                                field_counterpart.data_type()
                            ),
                        }));
                    }
                }
            }
        }

        Ok(())
    }

    /// Cheap.
    fn check_manifest_schema_matches_sorbet_schema(&self) -> CodecResult<()> {
        let any_static_chunks = self.col_chunk_is_static()?.any(|b| b);

        let sorbet_indexes = self
            .sorbet_schema
            .fields()
            .iter()
            .filter_map(|f| {
                let md = f.metadata();
                (md.get("rerun:kind").map(|s| s.as_str()) == Some("index"))
                    .then(|| md.contains_key("rerun:index_name").then_some(f))
                    .flatten()
            })
            .unique()
            .collect_vec();

        let sorbet_columns = self
            .sorbet_schema
            .fields()
            .iter()
            .filter(|f| f.metadata().get("rerun:kind").map(|s| s.as_str()) == Some("data"))
            .unique()
            .collect_vec();

        if any_static_chunks {
            // If there are any static chunks, then all components must have :has_static_data indexes.
            for column in &sorbet_columns {
                let md = column.metadata();
                let Some(component) = md.get("rerun:component") else {
                    return Err(crate::CodecError::from(ChunkError::Malformed {
                        reason: format!(
                            "column '{}' is missing rerun:component metadata",
                            column.name()
                        ),
                    }));
                };
                let descr = ComponentDescriptor {
                    archetype: md.get("rerun:archetype").map(|s| ArchetypeName::new(s)),
                    component: ComponentIdentifier::new(component),
                    component_type: md
                        .get("rerun:component_type")
                        .map(|s| ComponentType::new(s)),
                };
                let column_name = Self::compute_column_name(
                    None,
                    None,
                    Some(&descr),
                    None,
                    Some("has_static_data"),
                );

                self.data
                    .schema_ref()
                    .field_with_name(&column_name)
                    .map_err(|_err| {
                        crate::CodecError::from(ChunkError::Malformed {
                            reason: format!("static index '{column_name}' is missing"),
                        })
                    })?;
            }
        }

        let mut rrd_manifest_fields: HashMap<_, _> = self
            .data
            .schema_ref()
            .fields()
            .iter()
            .filter(|f| f.name().ends_with(":start") || f.name().ends_with(":end"))
            .map(|f| (f.name(), f))
            .collect();

        for sorbet_index in &sorbet_indexes {
            // We must account for the fact that names in the Sorbet schema are not normalized yet.
            let sorbet_index_name_normalized =
                Self::compute_column_name(None, None, None, Some(sorbet_index.name()), None);

            // All global indexes should have :start and :end columns of the right type.
            for suffix in ["start", "end"] {
                let field = rrd_manifest_fields.remove(&format!("{sorbet_index_name_normalized}:{suffix}"))
                    .ok_or_else(|| {
                        crate::CodecError::from(ChunkError::Malformed {
                            reason: format!(
                                "global index '{sorbet_index}' does not have matching `:{suffix}` field"
                            ),
                        })
                    })?;

                if sorbet_index.data_type() != field.data_type() {
                    return Err(crate::CodecError::from(ChunkError::Malformed {
                        reason: format!(
                            "global index '{}' is {} but '{}' is {}",
                            sorbet_index.name(),
                            sorbet_index.data_type(),
                            field.name(),
                            field.data_type()
                        ),
                    }));
                }
            }

            // All local indexes should exist and have :start and :end columns of the right type.
            for sorbet_column in &sorbet_columns {
                let md = sorbet_column.metadata();

                let Some(component) = md.get("rerun:component") else {
                    return Err(crate::CodecError::from(ChunkError::Malformed {
                        reason: format!(
                            "column '{}' is missing rerun:component metadata",
                            sorbet_column.name()
                        ),
                    }));
                };
                let descr = ComponentDescriptor {
                    archetype: md.get("rerun:archetype").map(|s| ArchetypeName::new(s)),
                    component: ComponentIdentifier::new(component),
                    component_type: md
                        .get("rerun:component_type")
                        .map(|s| ComponentType::new(s)),
                };

                for suffix in ["start", "end"] {
                    let column_name = Self::compute_column_name(
                        None,
                        None,
                        Some(&descr),
                        Some(sorbet_index.name()),
                        Some(suffix),
                    );

                    if md.get("rerun:is_static").map(|s| s.as_str()) == Some("true") {
                        // Static columns don't have :start nor :end columns… unless they exist
                        // both temporally and statically, something which is legal in Rerun, and
                        // will end up with a final Sorbet schema that declares those column as
                        // static (which is correct, since static overrides everything else), even
                        // though there are still traces of temporal data for that same column!
                        _ = rrd_manifest_fields.remove(&column_name);
                        continue;
                    }

                    let Some(field) = rrd_manifest_fields.remove(&column_name) else {
                        // Not all indexes have all components, that's fine.
                        continue;
                    };

                    if sorbet_index.data_type() != field.data_type() {
                        return Err(crate::CodecError::from(ChunkError::Malformed {
                            reason: format!(
                                "local index '{}' is {} but '{}' is {}",
                                sorbet_index.name(),
                                sorbet_index.data_type(),
                                field.name(),
                                field.data_type()
                            ),
                        }));
                    }
                }
            }
        }

        if !rrd_manifest_fields.is_empty() {
            return Err(crate::CodecError::from(ChunkError::Malformed {
                reason: format!(
                    "detected dangling indexes (present in manifest but not in Sorbet schema): {:?}",
                    rrd_manifest_fields.keys()
                ),
            }));
        }

        Ok(())
    }

    /// Costly.
    fn check_sorbet_schema_sha256_is_correct(&self) -> CodecResult<()> {
        let expected_sorbet_schema_sha256 = Self::compute_sorbet_schema_sha256(&self.sorbet_schema)
            .map_err(crate::CodecError::ArrowDeserialization)?;

        if self.sorbet_schema_sha256 != expected_sorbet_schema_sha256 {
            return Err(crate::CodecError::ArrowDeserialization(
                arrow::error::ArrowError::SchemaError(format!(
                    "invalid schema hash: expected {} but got {}",
                    expected_sorbet_schema_sha256
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                    self.sorbet_schema_sha256
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                )),
            ));
        }
        Ok(())
    }
}

// Fields
impl RrdManifest {
    pub const FIELD_CHUNK_ID: &str = "chunk_id";
    pub const FIELD_CHUNK_IS_STATIC: &str = "chunk_is_static";
    pub const FIELD_CHUNK_NUM_ROWS: &str = "chunk_num_rows";
    pub const FIELD_CHUNK_ENTITY_PATH: &str = "chunk_entity_path";
    pub const FIELD_CHUNK_BYTE_OFFSET: &str = "chunk_byte_offset";
    pub const FIELD_CHUNK_BYTE_SIZE: &str = "chunk_byte_size";

    pub fn field_chunk_id() -> Field {
        use re_log_types::external::re_types_core::Loggable as _;
        let nullable = false; // every chunk has an ID
        Field::new(Self::FIELD_CHUNK_ID, ChunkId::arrow_datatype(), nullable)
    }

    pub fn field_chunk_is_static() -> Field {
        let nullable = false; // every chunk is either static or temporal
        Field::new(
            Self::FIELD_CHUNK_IS_STATIC,
            arrow::datatypes::DataType::Boolean,
            nullable,
        )
    }

    pub fn field_chunk_num_rows() -> Field {
        let nullable = false; // every chunk has a number of rows
        Field::new(
            Self::FIELD_CHUNK_NUM_ROWS,
            arrow::datatypes::DataType::UInt64,
            nullable,
        )
    }

    pub fn field_chunk_entity_path() -> Field {
        let nullable = false; // every chunk has an entity path
        Field::new(
            Self::FIELD_CHUNK_ENTITY_PATH,
            arrow::datatypes::DataType::Utf8,
            nullable,
        )
    }

    pub fn field_chunk_byte_offset() -> Field {
        Self::any_byte_field(Self::FIELD_CHUNK_BYTE_OFFSET)
    }

    pub fn field_chunk_byte_size() -> Field {
        Self::any_byte_field(Self::FIELD_CHUNK_BYTE_SIZE)
    }

    pub fn field_index_start(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::any_index_field(timeline, timeline.datatype(), desc, "start")
    }

    pub fn field_index_end(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::any_index_field(timeline, timeline.datatype(), desc, "end")
    }

    pub fn field_index_num_rows(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::any_index_field(
            timeline,
            arrow::datatypes::DataType::UInt64,
            desc,
            "num_rows",
        )
    }

    pub fn field_index_has_data(timeline: &Timeline, desc: &ComponentDescriptor) -> Field {
        Self::any_index_field(
            timeline,
            arrow::datatypes::DataType::Boolean,
            Some(desc),
            "has_data",
        )
    }

    pub fn field_has_static_data(desc: &ComponentDescriptor) -> Field {
        let field_name =
            Self::compute_column_name(None, None, Some(desc), None, Some("has_static_data"));

        let mut metadata = std::collections::HashMap::default();
        metadata.extend(
            [
                Some(("rerun:index".to_owned(), "rerun:static".to_owned())), //
                desc.component_type.map(|component_type| {
                    (
                        "rerun:component_type".to_owned(),
                        component_type.full_name().to_owned(),
                    )
                }),
                desc.archetype
                    .as_ref()
                    .map(|name| ("rerun:archetype".to_owned(), name.full_name().to_owned())),
                Some(("rerun:component".to_owned(), desc.component.to_string())),
            ]
            .into_iter()
            .flatten(),
        );

        let nullable = false;
        Field::new(field_name, arrow::datatypes::DataType::Boolean, nullable)
            .with_metadata(metadata)
    }

    // ---

    fn any_index_field(
        timeline: &Timeline,
        datatype: arrow::datatypes::DataType,
        desc: Option<&ComponentDescriptor>,
        marker: &str,
    ) -> Field {
        let index_name = timeline.name();

        let field_name =
            Self::compute_column_name(None, None, desc, Some(index_name), Some(marker));

        let mut metadata = std::collections::HashMap::default();
        metadata.extend([("rerun:index".to_owned(), timeline.name().to_string())]);
        if let Some(desc) = desc {
            metadata.extend(
                [
                    desc.component_type.map(|component_type| {
                        (
                            "rerun:component_type".to_owned(),
                            component_type.full_name().to_owned(),
                        )
                    }),
                    desc.archetype
                        .as_ref()
                        .map(|name| ("rerun:archetype".to_owned(), name.full_name().to_owned())),
                    Some(("rerun:component".to_owned(), desc.component.to_string())),
                ]
                .into_iter()
                .flatten(),
            );
        }

        let nullable = true; // A) static B) not all chunks belong to all timelines
        Field::new(field_name, datatype, nullable).with_metadata(metadata)
    }

    fn any_byte_field(name: &str) -> Field {
        let nullable = false; // every chunk has an offset and size
        Field::new(name, arrow::datatypes::DataType::UInt64, nullable)
    }
}

// Column accessors
impl RrdManifest {
    /// Returns the raw Arrow data for the entity path column.
    pub fn col_chunk_entity_path_raw(&self) -> CodecResult<&StringArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_ENTITY_PATH;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<StringArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a StringArray",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the entity path column.
    ///
    /// This might incur interning costs, but is otherwise basically free.
    pub fn col_chunk_entity_path(&self) -> CodecResult<impl Iterator<Item = EntityPath>> {
        let col_raw = self.col_chunk_entity_path_raw()?;

        Ok(col_raw.iter().flatten().map(EntityPath::parse_forgiving))
    }

    /// Returns the raw Arrow data for the chunk ID column.
    pub fn col_chunk_id_raw(&self) -> CodecResult<&FixedSizeBinaryArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_ID;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<FixedSizeBinaryArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a FixedSizeBinaryArray",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the chunk ID column.
    ///
    /// This incurs a very cheap copy, but is otherwise basically free.
    pub fn col_chunk_id(&self) -> CodecResult<impl Iterator<Item = ChunkId>> {
        Ok(self
            .col_chunk_id_raw()?
            .iter()
            .flatten()
            .filter_map(|bytes| {
                let bytes: [u8; 16] = bytes
                    .try_into()
                    .inspect_err(|err| {
                        tracing::error!(
                            %err,
                            ?bytes,
                            "failed to parse chunk ID from fixed-size binary array"
                        );
                    })
                    .ok()?;
                Some(ChunkId::from_tuid(Tuid::from_bytes(bytes)))
            }))
    }

    /// Returns the raw Arrow data for the is-static column.
    pub fn col_chunk_is_static_raw(&self) -> CodecResult<&BooleanArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_IS_STATIC;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<BooleanArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a BooleanArray",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the is-static column.
    ///
    /// This is free.
    pub fn col_chunk_is_static(&self) -> CodecResult<impl Iterator<Item = bool>> {
        Ok(self.col_chunk_is_static_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the num-rows column.
    pub fn col_chunk_num_rows_raw(&self) -> CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_NUM_ROWS;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<UInt64Array>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a UInt64Array",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the num-rows column.
    ///
    /// This is free.
    pub fn col_chunk_num_rows(&self) -> CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_num_rows_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the byte-offset column.
    pub fn col_chunk_byte_offset_raw(&self) -> CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_BYTE_OFFSET;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<UInt64Array>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a UInt64Array",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the byte-offset column.
    ///
    /// This is free.
    pub fn col_chunk_byte_offset(&self) -> CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_byte_offset_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the byte-length column.
    pub fn col_chunk_byte_size_raw(&self) -> CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::FIELD_CHUNK_BYTE_SIZE;
        self.data
            .column_by_name(name)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot read column: '{name}' is missing from batch",),
                ))
            })?
            .downcast_array_ref::<UInt64Array>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(
                    format!("cannot downcast column: '{name}' is not a UInt64Array",),
                ))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the byte-length column.
    ///
    /// This is free.
    pub fn col_chunk_byte_size(&self) -> CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_byte_size_raw()?.iter().flatten())
    }
}
