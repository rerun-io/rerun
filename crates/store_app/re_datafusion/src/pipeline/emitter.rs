//! What turns one segment's chunks into rows.
//!
//! The driver owns delivery order, the safe horizon and the issuance window;
//! a [`SegmentRowEmitter`] owns the chunks of one segment and emits its rows
//! when the driver says how far it may go.

use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use re_dataframe::QueryExpression;
use re_dataframe::external::re_chunk::Chunk;
use re_log_types::{TimeInt, TimelineName};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::ApiResult;
use tokio::sync::mpsc::Sender;

use super::RowEmitterKind;
use super::columnar;
use crate::dataframe_query_common::IndexValuesMap;
use crate::dataframe_query_provider::segment_store::{IncrementalFlushOutcome, SegmentStore};

/// Turns one segment's delivered chunks into output rows, as far as the
/// driver's safe horizon allows.
///
/// The chunks live in a [`SegmentStore`]. Rows come from the columnar path
/// ([`super::columnar`]) when [`RowEmitterKind::Columnar`] is selected and
/// the query is one it covers, and from the store's `QueryHandle` otherwise.
///
/// Contract with the driver:
///
/// * [`Self::insert_chunk`] only ever sees planned, first-time deliveries;
///   the router drops unplanned and repeated chunks before they get here.
/// * [`Self::flush_incremental_to`] emits every row at or before `horizon`
///   that no earlier call emitted, and may drop chunks it no longer needs.
///   Each dropped chunk is reported exactly once in `gc.removed_chunks`, and
///   a kept chunk never is: the driver releases window bytes for exactly
///   those ids.
/// * [`Self::flush`] is called once, on a complete segment, after which the
///   emitter is dropped.
/// * [`Self::measured_bytes`] reflects inserts and GC; the driver reads it
///   around each insert and after the final [`Self::flush`].
pub struct SegmentRowEmitter {
    store: SegmentStore,

    /// The filtered index when rows come from the columnar path; `None`
    /// means they come from the store's `QueryHandle`.
    columnar_index: Option<TimelineName>,
}

impl SegmentRowEmitter {
    /// The emitter for one segment of a query.
    ///
    /// `index_values` carries the per-segment `using_index_values`, which
    /// the emitter applies to its own copy of the query.
    pub fn new(
        origin: re_uri::Origin,
        segment_id: SegmentId,
        query_expression: &QueryExpression,
        index_values: &IndexValuesMap,
        kind: RowEmitterKind,
    ) -> Self {
        let store = SegmentStore::new(origin, segment_id, query_expression, index_values);
        let columnar_index = match kind {
            RowEmitterKind::Columnar => columnar::columnar_index(&store.query_expression),
            RowEmitterKind::QueryHandle => None,
        };
        Self {
            store,
            columnar_index,
        }
    }

    /// Decoded bytes currently held for this segment.
    pub fn measured_bytes(&self) -> u64 {
        self.store.store_bytes()
    }

    /// Which kind of emitter actually produces this segment's rows, fixed at
    /// construction: [`RowEmitterKind::Columnar`] only when it was selected
    /// and the query is one the columnar path covers.
    pub fn kind(&self) -> RowEmitterKind {
        match self.columnar_index {
            Some(_) => RowEmitterKind::Columnar,
            None => RowEmitterKind::QueryHandle,
        }
    }

    /// Take ownership of one delivered chunk of this segment.
    pub fn insert_chunk(&mut self, chunk: &Arc<Chunk>) -> ApiResult<()> {
        self.store.insert_chunk(chunk)
    }

    /// Emit rows up to and including `horizon`, then drop what is no longer
    /// needed below it. `rows_sent` is the running total the driver compares
    /// against `limit_rows`.
    pub async fn flush_incremental_to(
        &mut self,
        horizon: TimeInt,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<IncrementalFlushOutcome> {
        if let Some(outcome) = self.store.begin_incremental_flush(horizon) {
            return Ok(outcome);
        }
        let rows_before = *rows_sent;
        self.emit(
            Some(horizon),
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await?;
        let gc = self.store.gc_up_to_horizon(horizon);
        Ok(IncrementalFlushOutcome {
            rows_emitted: *rows_sent - rows_before,
            gc,
        })
    }

    /// Emit everything not yet emitted.
    pub async fn flush(
        &mut self,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        self.emit(
            None,
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await
    }

    /// Emit every row up to `horizon` (`None` = to the end) not emitted yet.
    async fn emit(
        &mut self,
        horizon: Option<TimeInt>,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        match self.columnar_index {
            Some(index) => {
                columnar::emit(
                    &mut self.store,
                    index,
                    horizon,
                    projected_schema,
                    output_channel,
                    rows_sent,
                    limit_rows,
                )
                .await
            }
            None => {
                self.store
                    .emit_up_to(
                        horizon,
                        projected_schema,
                        output_channel,
                        rows_sent,
                        limit_rows,
                    )
                    .await
            }
        }
    }
}
