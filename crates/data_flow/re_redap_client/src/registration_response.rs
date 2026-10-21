//! Decode registration responses from the protocol.

use crate::{ApiError, ApiResult, TraceId};
use arrow::array::RecordBatch;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::RegisterWithDatasetResponse;
use re_protos::cloud::v1alpha1::ext::{
    DataSourceKind, RegisterWithDatasetDataframe, RegisterWithDatasetTaskDescriptor,
};
use re_protos::common::v1alpha1::DataframePart;
use re_protos::{TypeConversionError, missing_field};

/// Decode a `/RegisterWithDataset` response payload into task descriptors.
pub(crate) fn parse_task_descriptors(
    origin: &re_uri::Origin,
    trace_id: Option<TraceId>,
    data: Option<DataframePart>,
) -> ApiResult<Vec<RegisterWithDatasetTaskDescriptor>> {
    re_tracing::profile_function!();

    let response: RecordBatch = data
        .ok_or_else(|| {
            let err = missing_field!(RegisterWithDatasetResponse, "data");
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "missing field in /RegisterWithDataset response",
            )
        })?
        .try_into()
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "failed decoding /RegisterWithDataset response",
            )
        })?;

    // Validates the columns (existence, datatype, no nulls):
    let RegisterWithDatasetDataframe {
        rerun_segment_id,
        rerun_segment_layer,
        rerun_segment_type,
        rerun_storage_url,
        rerun_task_id,
    } = RegisterWithDatasetDataframe::try_from(response).map_err(|err| {
        ApiError::deserialization_quiver_from(
            origin,
            trace_id,
            err,
            "/RegisterWithDataset response",
        )
    })?;

    let segment_types = DataSourceKind::many_from_arrow(rerun_segment_type.as_arrow().as_ref())
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "failed parsing /RegisterWithDataset response",
            )
        })?;

    itertools::izip!(
        rerun_segment_layer.into_iter_owned(),
        rerun_segment_id.into_iter_owned(),
        segment_types,
        rerun_storage_url.into_iter_owned(),
        rerun_task_id.into_iter_owned()
    )
    .map(
        |(layer_name, segment_id, segment_type, storage_url, task_id)| {
            Ok(RegisterWithDatasetTaskDescriptor {
                layer_name,
                segment_id,
                segment_type,
                storage_url: url::Url::parse(&storage_url).map_err(|err| {
                    ApiError::deserialization_with_source(
                        origin,
                        trace_id,
                        TypeConversionError::UrlParseError(err),
                        "failed to parse /RegisterWithDataset response",
                    )
                })?,
                task_id,
            })
        },
    )
    .try_collect()
}
