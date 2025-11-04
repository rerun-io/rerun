use std::collections::HashMap;

use arrow::array::{BooleanArray, FixedSizeBinaryArray, StringArray, UInt64Array};
use itertools::Itertools as _;

use re_chunk::ChunkId;
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{EntityPath, StoreId};
use re_types_core::ComponentDescriptor;

use crate::{CodecError, CodecResult};

// ---

// TODO: we should remove these `kind` things. If Redap needs it then it should patch it in by itself.
// or maybe we just keep them, who cares...
// yeah nah im pretty sure this should go away.
//
// │ ┌──────────────────────────────────┬──────────────────────────────────┬──────────────────────────────────┬─────────────────┬───────────────────┬────────────────┐ │
// │ │ chunk_partition_id               ┆ chunk_entity_path                ┆ chunk_id                         ┆ chunk_is_static ┆ chunk_byte_offset ┆ chunk_byte_len │ │
// │ │ ---                              ┆ ---                              ┆ ---                              ┆ ---             ┆ ---               ┆ ---            │ │
// │ │ type: Utf8                       ┆ type: Utf8                       ┆ type: FixedSizeBinary[16]        ┆ type: bool      ┆ type: u64         ┆ type: u64      │ │
// │ │ kind: control                    ┆ kind: control                    ┆ kind: control                    ┆ kind: control   ┆                   ┆                │ │
// │ ╞══════════════════════════════════╪══════════════════════════════════╪══════════════════════════════════╪═════════════════╪═══════════════════╪════════════════╡ │

/// This is the message type that is passed in the footer of RRD streams.
///
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd route > all_my_recordings.rrd`,
/// would once again guarantee that only one footer is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
///
/// It is transported using the `MessageKind::End` tag.
///
/// This is an application-level type, the associated transport-level type can be found
/// over at [`re_protos::log_msg::v1alpha1::RrdFooter`].
pub struct RrdFooter {
    /// All the [`RrdManifest`]s that were found in this RRD footer.
    ///
    /// Each [`RrdManifest`] corresponds to one, and exactly one, RRD stream (i.e. recording).
    ///
    /// The order is unspecified.
    pub manifests: HashMap<StoreId, RrdManifest>,
}

/// The payload found in `RrdFooter`s.
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
/// ## A note on filtering
///
/// Always be on your toes when filtering rows out of an RRD manifest. Due to how the Rerun data
/// model works, removing rows (and therefore chunks) from a recording can also affect the number
/// of columns in that recording (because e.g. all the data for a specific entity path is gone),
/// which in turn will affect the Sorbet schema of the recording too.
///
/// Filtering RRD manifests is very non trivial and should only be done with great care.
#[derive(Clone)]
pub struct RrdManifest {
    /// The recording ID that was used to identify the original recording.
    ///
    /// This is extracted from the [`SetStoreInfo`] message of the associated RRD stream.
    pub store_id: StoreId,

    /// The Sorbet schema of the recording, following the usual merging and sorting rules.
    ///
    /// ⚠️ This is the Sorbet schema of the recording being indexed by this manifest, *not* the
    /// schema of [`Self::manifest`].
    pub sorbet_schema: arrow::datatypes::Schema,

    /// The SHA256 hash of the Sorbet schema of the associated RRD stream.
    ///
    /// This is always computed by sorting the fields of the schema by name first.
    pub sorbet_schema_sha256: [u8; 32],

    /// The actual manifest data, which catalogs every chunk in this recording.
    ///
    /// Each row in this dataframe describes a unique chunk (ID, offset, size, timeline & component stats, etc).
    /// This can be used to compute relevancy queries (latest-at, range, dataframe), without needing to load
    /// any of the actual data in memory.
    pub data: arrow::array::RecordBatch,
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
}

// TODO
impl RrdManifest {
    /// Checks the manifest for any traces of corruption.
    ///
    /// This is cheap to compute and is automatically performed when converting an `RrdManifest`
    /// from its transport-level to its application-level representation (and vice-versa).
    ///
    /// See [`Self::sanity_check_heavy`] for a more costly version that is not suitable to use in
    /// production, but is very useful in e.g. tests.
    pub fn sanity_check_cheap(&self) -> CodecResult<()> {
        self.static_columns_are_correct()?;
        Ok(())
    }

    /// Checks the manifest for any traces of corruption.
    ///
    /// This is quite costly and therefore should not be used on the happy production path.
    /// Prefer [`Self::sanity_check_cheap`] for that instead.
    pub fn sanity_check_heavy(&self) -> CodecResult<()> {
        self.schema_sha256_is_correct()?;
        Ok(())
    }

    // TODO: all :start columns should have a matching :end
    // TODO: (extended) all :start,:end pairs should have a matching :len

    // TODO: reminder of what we're workng with here (this prob belong in the docs)
    // chunk_byte_len: u64
    // chunk_byte_offset: u64
    // chunk_entity_path: Utf8 [
    //     rerun:kind:control
    // ]
    // chunk_id: FixedSizeBinary[16] [
    //     rerun:kind:control
    // ]
    // chunk_is_static: bool [
    //     rerun:kind:control
    // ]
    // chunk_partition_id: Utf8 [
    //     rerun:kind:control
    // ]
    // frame_nr:end: i64 [
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:end
    // ]
    // frame_nr:example_MyPoints:colors:end: i64 [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:colors
    //     rerun:component_descriptor:example.MyPoints:colors
    //     rerun:component_type:example.MyColor
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:end
    // ]
    // frame_nr:example_MyPoints:colors:has_data: bool [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:colors
    //     rerun:component_descriptor:example.MyPoints:colors
    //     rerun:component_type:example.MyColor
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:has_data
    // ]
    // frame_nr:example_MyPoints:colors:start: i64 [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:colors
    //     rerun:component_descriptor:example.MyPoints:colors
    //     rerun:component_type:example.MyColor
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:start
    // ]
    // frame_nr:example_MyPoints:points:end: i64 [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:points
    //     rerun:component_descriptor:example.MyPoints:points
    //     rerun:component_type:example.MyPoint
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:end
    // ]
    // frame_nr:example_MyPoints:points:has_data: bool [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:points
    //     rerun:component_descriptor:example.MyPoints:points
    //     rerun:component_type:example.MyPoint
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:has_data
    // ]
    // frame_nr:example_MyPoints:points:start: i64 [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:points
    //     rerun:component_descriptor:example.MyPoints:points
    //     rerun:component_type:example.MyPoint
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:start
    // ]
    // frame_nr:start: i64 [
    //     rerun:index:frame_nr
    //     rerun:index_kind:sequence
    //     rerun:index_marker:start
    // ]
    // static:example_MyPoints:colors:has_data: bool [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:colors
    //     rerun:component_descriptor:example.MyPoints:colors
    //     rerun:component_type:example.MyColor
    //     rerun:index:static
    // ]
    // static:example_MyPoints:labels:has_data: bool [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:labels
    //     rerun:component_descriptor:example.MyPoints:labels
    //     rerun:component_type:example.MyLabel
    //     rerun:index:static
    // ]
    // static:example_MyPoints:points:has_data: bool [
    //     rerun:archetype:example.MyPoints
    //     rerun:component:example.MyPoints:points
    //     rerun:component_descriptor:example.MyPoints:points
    //     rerun:component_type:example.MyPoint
    //     rerun:index:static
    // ]
    // ┌───────────────────────────────────────────┬──────────────────────────────────┬──────────────────────────────────┐
    // │ chunk_partition_id                        ┆ my_recording                     ┆ my_recording                     │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ chunk_entity_path                         ┆ /my_entity                       ┆ /my_entity                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ chunk_id                                  ┆ 00000000000000010000000000000001 ┆ 00000000000000010000000000000002 │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ chunk_is_static                           ┆ false                            ┆ true                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ chunk_byte_offset                         ┆ 104                              ┆ 1514                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ chunk_byte_len                            ┆ 1394                             ┆ 947                              │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ static:example_MyPoints:colors:has_data   ┆ false                            ┆ false                            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ static:example_MyPoints:labels:has_data   ┆ false                            ┆ true                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ static:example_MyPoints:points:has_data   ┆ false                            ┆ false                            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:start                            ┆ 10                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:end                              ┆ 40                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:colors:start    ┆ 10                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:colors:end      ┆ 40                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:colors:has_data ┆ true                             ┆ false                            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:points:start    ┆ 10                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:points:end      ┆ 40                               ┆ null                             │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ frame_nr:example_MyPoints:points:has_data ┆ true                             ┆ false                            │
    // └───────────────────────────────────────────┴──────────────────────────────────┴──────────────────────────────────┘

    /// Cheap.
    fn static_columns_are_correct(&self) -> CodecResult<()> {
        _ = self.col_chunk_partition_id()?;
        _ = self.col_chunk_id()?;
        _ = self.col_chunk_is_static()?;
        _ = self.col_chunk_entity_path()?;
        _ = self.col_chunk_byte_offset()?;
        _ = self.col_chunk_byte_len()?;
        Ok(())
    }

    /// Cheap.
    fn schema_matches_data(&self) -> CodecResult<()> {
        // TODO: okay that one is gonna be quite a bit more annoying
        Ok(())
    }

    /// Costly.
    fn schema_sha256_is_correct(&self) -> CodecResult<()> {
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

    fn rrd_manifest_is_correct(&self) -> CodecResult<()> {
        // TODO: as in, actually reencode the data. does that make sense / help?
        Ok(())
    }
}

impl RrdManifest {
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

// TODO: nah but srsly though, partition-id column needs to go

// TODO: maybe we need some schema checks all over the place like we do for ext types?
// TODO: maybe the ToApplication impl could check that things aren't completely outta whack or something -> yes
impl RrdManifest {
    pub const CHUNK_PARTITION_ID_FIELD_NAME: &str = "chunk_partition_id";
    pub const CHUNK_ID_FIELD_NAME: &str = "chunk_id";
    pub const CHUNK_IS_STATIC_FIELD_NAME: &str = "chunk_is_static";
    pub const CHUNK_ENTITY_PATH_FIELD_NAME: &str = "chunk_entity_path";
    pub const CHUNK_BYTE_OFFSET_FIELD_NAME: &str = "chunk_byte_offset";
    pub const CHUNK_BYTE_LEN_FIELD_NAME: &str = "chunk_byte_len";
}

// Accessors
impl RrdManifest {
    /// Returns the raw Arrow data for the partition ID column.
    pub fn col_chunk_partition_id_raw(&self) -> CodecResult<&StringArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::CHUNK_PARTITION_ID_FIELD_NAME;
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

    /// Returns an iterator over the decoded Arrow data for the partition ID column.
    ///
    /// This is free.
    pub fn col_chunk_partition_id(&self) -> CodecResult<impl Iterator<Item = &str>> {
        Ok(self.col_chunk_partition_id_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the entity path column.
    pub fn col_chunk_entity_path_raw(&self) -> CodecResult<&StringArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::CHUNK_ENTITY_PATH_FIELD_NAME;
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
        let name = Self::CHUNK_ID_FIELD_NAME;
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
        let name = Self::CHUNK_IS_STATIC_FIELD_NAME;
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

    /// Returns the raw Arrow data for the byte-offset column.
    pub fn col_chunk_byte_offset_raw(&self) -> CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::CHUNK_BYTE_OFFSET_FIELD_NAME;
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
    pub fn col_chunk_byte_len_raw(&self) -> CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        let name = Self::CHUNK_BYTE_LEN_FIELD_NAME;
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
    pub fn col_chunk_byte_len(&self) -> CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_byte_len_raw()?.iter().flatten())
    }
}
