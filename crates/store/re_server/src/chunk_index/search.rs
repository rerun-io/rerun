use std::sync::Arc;

use arrow::array::{RecordBatch, StringArray};
use datafusion::common::ScalarValue;
use futures::{Stream, StreamExt as _};
use itertools::Itertools as _;
use lance_index::scalar::FullTextSearchQuery;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_protos::cloud::v1alpha1::ext::{IndexQueryProperties, SearchDatasetRequest};
use re_protos::common::v1alpha1::ext::ScanParameters;
use tracing::info;

use crate::chunk_index::{FIELD_INSTANCE, Index};
use crate::store::Error as StoreError;

pub async fn search_index(
    index: Arc<Index>,
    request: SearchDatasetRequest,
) -> Result<impl Stream<Item = Result<RecordBatch, StoreError>> + use<>, StoreError> {
    let lance_dataset = index.lance_dataset.get();

    if request.query.columns().len() != 1 && request.query.num_rows() != 1 {
        return Err(StoreError::IndexingError(
            "Query must have exactly one row and one column".to_owned(),
        ));
    }

    let query_data = request.query.column(0);

    let length_zero = request.scan_parameters.limit_len == Some(0);

    let stream = match request.properties {
        IndexQueryProperties::Inverted => {
            let q = query_data.try_downcast_array_ref::<StringArray>()?.value(0);

            let fts =
                FullTextSearchQuery::new(q.to_owned()).with_column(FIELD_INSTANCE.to_owned())?;

            let mut scanner = &mut lance_dataset.scan();
            scanner = scanner.full_text_search(fts)?;
            apply_parameters(scanner, request.scan_parameters).await?;
            scanner.try_into_stream().await?
        }

        IndexQueryProperties::Vector { top_k } => {
            let mut scanner = &mut lance_dataset.scan();
            scanner = scanner.nearest(FIELD_INSTANCE, query_data, top_k as usize)?;
            apply_parameters(scanner, request.scan_parameters).await?;

            scanner.try_into_stream().await?
        }

        IndexQueryProperties::Btree => {
            let q = ScalarValue::try_from_array(query_data, 0)?;

            let scanner = &mut lance_dataset.scan();
            {
                use datafusion::prelude::*;
                scanner.filter_expr(col(FIELD_INSTANCE).eq(lit(q)));
            }

            apply_parameters(scanner, request.scan_parameters).await?;

            scanner.try_into_stream().await?
        }
    };

    use lance::io::RecordBatchStream as _;

    // To find the schema of the query results, we do a query with a limit of 0.
    // However, such a query results in an empty stream. In that case we force
    // creation of an empty record batch with the right schema, and return that
    // instead.
    //
    // Note, it's important we do this here because the lance scanner actually
    // mutates the schema based on the type of search being done.
    let stream = if length_zero {
        let rb = RecordBatch::new_empty(stream.schema());
        tokio_util::either::Either::Left(tokio_stream::iter(vec![Ok(rb)]))
    } else {
        tokio_util::either::Either::Right(stream)
    };

    let stream = stream.map(|s| s.map_err(Into::into));
    Ok(stream)
}

// Borrowed from redap's ScannerExt
async fn apply_parameters(
    scanner: &mut lance::dataset::scanner::Scanner,
    parameters: ScanParameters,
) -> Result<(), StoreError> {
    let ScanParameters {
        columns,
        on_missing_columns: _,
        filter,
        limit_offset,
        limit_len,
        order_by,
        explain_plan,
        explain_filter,
    } = parameters;

    // `project_from_schema` added in https://github.com/rerun-io/lance/pull/10
    // Use regular projection instead for now.
    //   let projected_schema = lance_dataset.schema().project(&columns)?;
    //   scanner.project_from_schema(&projected_schema)?;
    scanner.project(&columns)?;

    if let Some(filter) = filter.filter(|f| !f.is_empty()) {
        let filter =
            lance::io::exec::Planner::new(scanner.schema().await?).parse_filter(&filter)?;
        match scanner.get_filter()? {
            Some(existing_filter) => {
                scanner.filter_expr(existing_filter.and(filter));
            }
            None => {
                scanner.filter_expr(filter);
            }
        }
    }

    scanner.limit(limit_len, limit_offset)?;

    if !order_by.is_empty() {
        let order_by = order_by
            .into_iter()
            .map(|order_by| lance::dataset::scanner::ColumnOrdering {
                ascending: !order_by.descending,
                nulls_first: !order_by.nulls_last,
                column_name: order_by.column_name,
            })
            .collect_vec();
        scanner.order_by(Some(order_by))?;
    }

    if explain_plan {
        match scanner.explain_plan(false).await {
            Ok(plan) => {
                info!(plan);
            }
            Err(err) => {
                info!("Failed to compute execution plan: {err:#}");
            }
        }
    }

    if explain_filter {
        match scanner.get_filter() {
            Ok(Some(filter)) => {
                info!(%filter);
            }
            Ok(_) => {
                info!("No filter set");
            }
            Err(err) => {
                info!("Failed to fetch current filter: {err:#}");
            }
        }
    }

    Ok(())
}
