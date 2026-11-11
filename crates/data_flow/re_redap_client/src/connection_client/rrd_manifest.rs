use re_chunk_index::RawRrdManifest;
use re_log_encoding::{Decodable as _, ToApplication as _};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    ETag, RrdManifestKey as RrdManifestKeyExt, SOURCE_CHANGED_MESSAGE, url_strip_query,
};
use re_protos::cloud::v1alpha1::{GetRrdManifestResponse, RrdManifestKey};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiErrorKind, ApiResponseStream, ApiResult, TraceId};

/// Checks that a `Content-Range` value describes the exact inclusive byte range expected by the
/// request and, when present, a complete object length greater than the range end.
fn content_range_matches(value: &str, expected_start: u64, expected_end: u64) -> bool {
    let Some((unit, value)) = value.split_once(' ') else {
        return false;
    };
    let Some((range, complete_length)) = value.split_once('/') else {
        return false;
    };
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };

    unit.eq_ignore_ascii_case("bytes")
        && start.parse::<u64>() == Ok(expected_start)
        && end.parse::<u64>() == Ok(expected_end)
        && (complete_length == "*"
            || complete_length
                .parse::<u64>()
                .is_ok_and(|complete_length| expected_end < complete_length))
}

async fn fetch_rrd_manifest_via_key(
    origin: &re_uri::Origin,
    manifest_key: RrdManifestKey,
    segment_id: &SegmentId,
    trace_id: Option<TraceId>,
) -> ApiResult<RawRrdManifest> {
    let RrdManifestKeyExt {
        location,
        layer,
        etag,
        direct_url,
        store_id,
    } = manifest_key.try_into().map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "invalid /GetRrdManifest manifest key",
        )
    })?;

    let Some(direct_url) = direct_url else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest key carries no direct_url to fetch",
        ));
    };

    let Some(range_end) = location
        .byte_span
        .len
        .checked_sub(1)
        .and_then(|length_minus_one| location.byte_span.start.checked_add(length_minus_one))
    else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest byte range is empty or overflows u64",
        ));
    };

    let mut request = ehttp::Request::get(direct_url.as_str()).with_timeout(None);
    request.headers.insert(
        http::header::RANGE.as_str(),
        format!("bytes={}-{range_end}", location.byte_span.start),
    );
    let expected_etag = etag.filter(|etag| !etag.is_empty());
    if let Some(etag) = expected_etag.as_ref().and_then(ETag::as_if_match) {
        request
            .headers
            .insert(http::header::IF_MATCH.as_str(), etag);
    }

    cfg_select! {
        target_family = "wasm" => {
            let response = re_async::spawn_local_with_result(ehttp::fetch_async(request))
                .await
                .unwrap_or_else(|_| Err("HTTP request was canceled".to_owned()));
        }
        _ => {
            let response = ehttp::fetch_async(request).await;
        }
    }

    let redacted_url = url_strip_query(direct_url.as_str());

    // The URL is context, not part of the sentence: it goes on a detail line, so that it ends up
    // in the details section instead of stretching the summary.
    let with_url = |what: &str| re_error::format_with_details(what, format!("URL: {redacted_url}"));

    let response = response.map_err(|err| {
        let err = err.replace(direct_url.as_str(), redacted_url);
        ApiError::connection_with_source(
            origin,
            trace_id,
            std::io::Error::other(err),
            with_url("failed to fetch RRD manifest directly"),
        )
    })?;

    let source_changed_error = || {
        ApiError::http_status_with_source(
            origin,
            trace_id,
            http::StatusCode::PRECONDITION_FAILED.as_u16(),
            std::io::Error::other(SOURCE_CHANGED_MESSAGE),
            with_url("failed to fetch RRD manifest directly"),
        )
    };

    // HTTP permits servers to ignore `Range` and return `200 OK`, but this path requires the
    // requested range to be honored to avoid downloading or decoding the full RRD object.
    if response.status != http::StatusCode::PARTIAL_CONTENT.as_u16() {
        return if response.status == http::StatusCode::PRECONDITION_FAILED.as_u16() {
            Err(source_changed_error())
        } else {
            let message =
                re_error::StructuredError::from_summary("failed to fetch RRD manifest directly")
                    .with_details(layer.as_deref().map(|layer| format!("Layer: {layer}")))
                    .with_detail(format!("URL: {redacted_url}"));
            Err(ApiError::http_status(
                origin,
                trace_id,
                response.status,
                message.to_string(),
            ))
        };
    }

    let content_range = response.headers.get(http::header::CONTENT_RANGE.as_str());
    if !content_range.is_some_and(|content_range| {
        content_range_matches(content_range, location.byte_span.start, range_end)
    }) {
        return Err(ApiError::with_kind_and_source(
            origin,
            ApiErrorKind::InvalidServer,
            trace_id,
            std::io::Error::other(format!(
                "invalid Content-Range: {}",
                content_range.unwrap_or("missing")
            )),
            with_url("failed to fetch RRD manifest directly"),
        ));
    }

    let expected_length = usize::try_from(location.byte_span.len).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "direct RRD manifest is too large for this client",
        )
    })?;
    if response.bytes.len() != expected_length {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            with_url(&format!(
                "direct RRD manifest response had {} bytes, expected {expected_length}",
                response.bytes.len()
            )),
        ));
    }

    let rrd_footer = re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(&response.bytes)
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                with_url("failed decoding direct RRD footer"),
            )
        })?;

    let rrd_manifest = rrd_footer
        .into_segment_manifest(store_id.as_ref(), segment_id)
        .ok_or_else(|| {
            let what = match &store_id {
                Some(store_id) => format!("store {store_id}"),
                None => format!("segment {segment_id}"),
            };
            ApiError::deserialization(
                origin,
                trace_id,
                with_url(&format!(
                    "direct RRD footer did not contain a manifest for {what}"
                )),
            )
        })?;

    let raw = rrd_manifest.to_application(()).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            with_url("failed parsing direct RRD manifest"),
        )
    })?;

    let Some(layer) = layer else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest key carries no layer",
        ));
    };

    // `FetchChunks` needs the same `chunk_partition_id`/`rerun_partition_layer`/`chunk_key`
    // columns the inline `GetRrdManifest` path serves; the raw manifest alone doesn't carry them.
    // [`re_log_encoding::HubRrdManifest::try_from_raw`] handles this.
    let hub = re_log_encoding::HubRrdManifest::try_from_raw(
        &raw,
        segment_id,
        &layer,
        &location.url,
        location.object_store_config.as_deref(),
        expected_etag.as_ref(),
        None,
    )
    .map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            with_url("failed hub-extending direct RRD manifest"),
        )
    })?;

    // convert back to raw manifest so it can be interpreted
    // with no changes by downstream users
    Ok(hub.into_raw())
}

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Stream the [`RawRrdManifest`] parts of a recording as they arrive from the server.
    ///
    /// Each item in the returned stream is a manifest part (a slice of the full manifest).
    /// Use [`RawRrdManifest::merge`] to combine parts because they may have different, overlapping schemas.
    ///
    /// The server may answer with manifest keys instead of inline manifests, in which case each
    /// part is fetched directly from the object store. In a browser that direct fetch only works
    /// when the bucket has a CORS configuration that allows it (see
    /// <https://rerun.io/docs/hub/bucket-configuration#cors>), so wasm clients fall back to
    /// server-provided manifests when the first part fails.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_rrd_manifest_stream(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<ApiResponseStream<RawRrdManifest>> {
        if !cfg!(target_family = "wasm") {
            return self
                .get_rrd_manifest_stream_impl(dataset_id, segment_id, true)
                .await;
        }

        // In a browser, direct object-store reads only work when the bucket's CORS
        // configuration allows them, which cannot be assumed: if it fails,
        // ask for server-provided manifests instead.
        let mut stream = self
            .get_rrd_manifest_stream_impl(dataset_id, segment_id.clone(), true)
            .await?;
        let trace_id = stream.trace_id();
        match stream.next().await {
            Some(Ok(first)) => Ok(ApiResponseStream::new(
                self.origin.clone(),
                futures::stream::iter([Ok(first)]).chain(stream), // NOLINT: Stream::chain, not Iterator::chain
                trace_id,
            )),
            // Browsers report a CORS block as an opaque network failure, so `Connection` is
            // the narrowest kind that covers it. This costs a fallback even on other
            // types of network failures.
            Some(Err(err)) if err.kind == ApiErrorKind::Connection => {
                re_log::warn_once!(
                    "{}",
                    re_error::format_with_details(
                        "Failed to fetch an RRD footer directly from the object store, \
                         falling back to slower server-provided manifests. \
                         This usually means the bucket is missing a CORS configuration that \
                         allows the Web Viewer to read it — see \
                         https://rerun.io/docs/hub/bucket-configuration#cors for how to set it up.",
                        err.to_string(),
                    )
                );
                self.get_rrd_manifest_stream_impl(dataset_id, segment_id, false)
                    .await
            }
            Some(Err(err)) => Err(err),
            None => Ok(stream),
        }
    }

    async fn get_rrd_manifest_stream_impl(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        generate_direct_urls: bool,
    ) -> ApiResult<ApiResponseStream<RawRrdManifest>> {
        let request_segment_id = segment_id.clone();
        let client = self.inner().clone();
        let response = crate::rpc_retry::retry(|| {
            let mut client = client.clone();
            let request_segment_id = request_segment_id.clone();
            async move {
                crate::rpc_retry::open_stream(
                    client
                        .get_rrd_manifest(
                            tonic::Request::new(
                                re_protos::cloud::v1alpha1::GetRrdManifestRequest {
                                    segment_id: Some(request_segment_id.clone().into()),
                                    generate_direct_urls,
                                    client_capabilities: vec![
                                        re_protos::capabilities::DATASET_RRD_MANIFEST_KEY_STORE_ID
                                            .to_owned(),
                                    ],
                                },
                            )
                            .with_entry_id(dataset_id),
                        )
                        .await?,
                )
                .await
            }
        })
        .await
        .map_err(|err| ApiError::tonic(&self.origin, err, "/GetRrdManifest"))?;

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/GetRrdManifest",
        )
        .inspect_first(|response| {
            crate::dataset_revisions().observe_meta(response.meta.as_ref());
        });
        let trace_id = stream.trace_id();
        let origin = self.origin.clone();
        let stream = stream.then(move |resp| {
            let segment_id = segment_id.clone();
            let origin = origin.clone();
            async move {
                let GetRrdManifestResponse {
                    rrd_manifest,
                    manifest_key,
                    ..
                } = resp?;

                match (rrd_manifest, manifest_key) {
                    (Some(rrd_manifest), None) => rrd_manifest.to_application(()).map_err(|err| {
                        ApiError::deserialization_with_source(
                            &origin,
                            trace_id,
                            err,
                            "failed parsing inline /GetRrdManifest response",
                        )
                    }),
                    (None, Some(manifest_key)) => {
                        fetch_rrd_manifest_via_key(&origin, manifest_key, &segment_id, trace_id)
                            .await
                    }
                    (None, None) => Err(ApiError::deserialization(
                        &origin,
                        trace_id,
                        "/GetRrdManifest response contained neither a manifest nor a manifest key",
                    )),
                    (Some(_), Some(_)) => Err(ApiError::deserialization(
                        &origin,
                        trace_id,
                        "/GetRrdManifest response contained both a manifest and a manifest key",
                    )),
                }
            }
        });
        Ok(ApiResponseStream::new(
            self.origin.clone(),
            stream,
            trace_id,
        ))
    }

    /// Get the full [`RawRrdManifest`] of a recording, combined from all stream parts.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_rrd_manifest(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<RawRrdManifest> {
        let stream = self.get_rrd_manifest_stream(dataset_id, segment_id).await?;
        let trace_id = stream.trace_id();

        futures::pin_mut!(stream);

        let mut rrd_manifest_parts = Vec::new();
        while let Some(part) = stream.next().await {
            rrd_manifest_parts.push(part?);
        }

        let Some(first) = rrd_manifest_parts.first() else {
            return Err(ApiError::deserialization(
                &self.origin,
                trace_id,
                "failed to parse the response for /GetRrdManifest (no data)",
            ));
        };
        if rrd_manifest_parts.len() == 1 {
            return Ok(rrd_manifest_parts.pop().expect("length was checked"));
        }

        RawRrdManifest::merge(first.store_id.clone(), rrd_manifest_parts).map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed merging /GetRrdManifest response parts",
            )
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_types_core::LayerName;

    use super::*;

    #[test]
    fn direct_rrd_manifest_response_metadata_is_validated() {
        assert!(content_range_matches("bytes 10-19/100", 10, 19));
        assert!(content_range_matches("BYTES 10-19/*", 10, 19));
        assert!(!content_range_matches("bytes 11-20/100", 10, 19));
        assert!(!content_range_matches("bytes 10-19/19", 10, 19));
        assert!(!content_range_matches("invalid", 10, 19));
    }

    #[tokio::test]
    async fn direct_rrd_manifest_fetch_uses_range_and_etag() {
        use re_log_encoding::{Encodable as _, ToTransport as _};

        let raw_manifest = RawRrdManifest::build_in_memory_from_chunks(
            re_log_types::StoreId::recording("test", "recording"),
            std::iter::empty::<&re_chunk::Chunk>(),
        )
        .unwrap();
        let other_manifest = RawRrdManifest::build_in_memory_from_chunks(
            re_log_types::StoreId::recording("test", "other"),
            std::iter::empty::<&re_chunk::Chunk>(),
        )
        .unwrap();
        let rrd_footer = re_log_encoding::RrdFooter {
            manifests: std::collections::HashMap::from([
                // Add a second manifest to ensure we pick out the right one.
                (other_manifest.store_id.clone(), other_manifest),
                (raw_manifest.store_id.clone(), raw_manifest.clone()),
            ]),
        };
        let mut manifest_bytes = Vec::new();
        rrd_footer
            .to_transport(())
            .unwrap()
            .to_rrd_bytes(&mut manifest_bytes)
            .unwrap();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let offset = 123;
        let range_end = offset + manifest_bytes.len() as u64 - 1;
        let response_bytes = manifest_bytes.clone();
        let server_thread = std::thread::Builder::new()
            .name("direct-manifest-test-server".to_owned())
            .spawn(move || {
                let request = server.recv().unwrap();
                let header = |name| {
                    request
                        .headers()
                        .iter()
                        .find(|header| header.field.equiv(name))
                        .map(|header| header.value.as_str().to_owned())
                };
                let range = header(http::header::RANGE.as_str());
                let if_match = header(http::header::IF_MATCH.as_str());
                request
                    .respond(
                        tiny_http::Response::from_data(response_bytes)
                            .with_status_code(tiny_http::StatusCode(
                                http::StatusCode::PARTIAL_CONTENT.as_u16(),
                            ))
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    http::header::CONTENT_RANGE.as_str(),
                                    format!("bytes {offset}-{range_end}/{}", range_end + 1),
                                )
                                .unwrap(),
                            )
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    http::header::ETAG.as_str(),
                                    "\"different-etag\"",
                                )
                                .unwrap(),
                            ),
                    )
                    .unwrap();
                (range, if_match)
            })
            .unwrap();

        let manifest_key = RrdManifestKey {
            location: Some(re_protos::cloud::v1alpha1::RrdChunkLocation {
                url: Some("s3://bucket/recording.rrd".to_owned()),
                offset: Some(offset),
                length: Some(manifest_bytes.len() as u64),
                object_store_config: None,
            }),
            layer: Some("base".to_owned()),
            etag: Some("\"registered-etag\"".to_owned()),
            direct_url: Some(format!(
                "http://{address}/recording.rrd?X-Amz-Signature=secret"
            )),
            store_id: None,
        };

        let segment_id = SegmentId::new("recording".to_owned());
        let layer = LayerName::base();
        let canonical_url = url::Url::parse("s3://bucket/recording.rrd").expect("valid s3 url");
        let etag = ETag::new("\"registered-etag\"");

        let fetched =
            fetch_rrd_manifest_via_key(&re_uri::Origin::test(), manifest_key, &segment_id, None)
                .await
                .expect("direct manifest fetch succeeds");
        assert_eq!(fetched.store_id, raw_manifest.store_id);
        assert_eq!(
            fetched.sorbet_schema_sha256,
            raw_manifest.sorbet_schema_sha256
        );

        let expected = re_log_encoding::HubRrdManifest::try_from_raw(
            &raw_manifest,
            &segment_id,
            &layer,
            &canonical_url,
            None,
            Some(&etag),
            None,
        )
        .expect("hub-extending a zero-row in-memory manifest cannot fail")
        .into_raw();
        assert_eq!(fetched.data, expected.data);
        for hub_column in [
            re_log_encoding::HubRrdManifest::COLUMN_CHUNK_PARTITION_ID.name,
            re_log_encoding::HubRrdManifest::COLUMN_RERUN_PARTITION_LAYER.name,
            re_log_encoding::HubRrdManifest::COLUMN_CHUNK_KEY.name,
        ] {
            assert!(
                fetched.data.column_by_name(hub_column).is_some(),
                "fetched manifest is missing hub column '{hub_column}'"
            );
        }

        let (range, if_match) = server_thread.join().unwrap();
        assert_eq!(
            range.as_deref(),
            Some(format!("bytes={offset}-{range_end}").as_str())
        );
        assert_eq!(if_match.as_deref(), Some("\"registered-etag\""));
    }

    #[tokio::test]
    async fn direct_rrd_manifest_fetch_selects_the_keyed_store() {
        use re_log_encoding::{Encodable as _, ToTransport as _};

        // Both stores share the segment's recording id, so only the full store id tells them
        // apart.
        let first = re_log_types::StoreId::recording("first_app", "recording");
        let second = re_log_types::StoreId::recording("second_app", "recording");
        let manifest = |store_id: &re_log_types::StoreId| {
            RawRrdManifest::build_in_memory_from_chunks(
                store_id.clone(),
                std::iter::empty::<&re_chunk::Chunk>(),
            )
            .unwrap()
        };
        let rrd_footer = re_log_encoding::RrdFooter {
            manifests: std::collections::HashMap::from([
                (first.clone(), manifest(&first)),
                (second.clone(), manifest(&second)),
            ]),
        };
        let mut manifest_bytes = Vec::new();
        rrd_footer
            .to_transport(())
            .unwrap()
            .to_rrd_bytes(&mut manifest_bytes)
            .unwrap();

        let store_ids = [Some(first.clone()), Some(second.clone()), None];
        let num_requests = store_ids.len();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let range_end = manifest_bytes.len() as u64 - 1;
        let response_bytes = manifest_bytes.clone();
        let server_thread = std::thread::Builder::new()
            .name("direct-manifest-test-server".to_owned())
            .spawn(move || {
                for _ in 0..num_requests {
                    let request = server.recv().unwrap();
                    request
                        .respond(
                            tiny_http::Response::from_data(response_bytes.clone())
                                .with_status_code(tiny_http::StatusCode(
                                    http::StatusCode::PARTIAL_CONTENT.as_u16(),
                                ))
                                .with_header(
                                    tiny_http::Header::from_bytes(
                                        http::header::CONTENT_RANGE.as_str(),
                                        format!("bytes 0-{range_end}/{}", range_end + 1),
                                    )
                                    .unwrap(),
                                ),
                        )
                        .unwrap();
                }
            })
            .unwrap();

        let segment_id = SegmentId::new("recording".to_owned());
        for store_id in store_ids {
            let manifest_key = RrdManifestKey {
                location: Some(re_protos::cloud::v1alpha1::RrdChunkLocation {
                    url: Some("s3://bucket/recording.rrd".to_owned()),
                    offset: Some(0),
                    length: Some(manifest_bytes.len() as u64),
                    object_store_config: None,
                }),
                layer: Some("base".to_owned()),
                etag: None,
                direct_url: Some(format!("http://{address}/recording.rrd")),
                store_id: store_id.clone().map(Into::into),
            };

            let fetched = fetch_rrd_manifest_via_key(
                &re_uri::Origin::test(),
                manifest_key,
                &segment_id,
                None,
            )
            .await
            .expect("direct manifest fetch succeeds");

            match store_id {
                Some(store_id) => assert_eq!(fetched.store_id, store_id),
                // A legacy key picks either store: both match the segment id.
                None => assert_eq!(fetched.store_id.recording_id().as_str(), "recording"),
            }
        }

        server_thread.join().unwrap();
    }

    #[tokio::test]
    async fn direct_rrd_manifest_fetch_maps_precondition_failure_without_leaking_query() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let server_thread = std::thread::Builder::new()
            .name("direct-manifest-test-server".to_owned())
            .spawn(move || {
                let request = server.recv().unwrap();
                request
                    .respond(tiny_http::Response::empty(tiny_http::StatusCode(
                        http::StatusCode::PRECONDITION_FAILED.as_u16(),
                    )))
                    .unwrap();
            })
            .unwrap();

        let manifest_key = RrdManifestKey {
            location: Some(re_protos::cloud::v1alpha1::RrdChunkLocation {
                url: Some("s3://bucket/manifest".to_owned()),
                offset: Some(0),
                length: Some(1),
                object_store_config: None,
            }),
            layer: None,
            etag: Some("\"old-etag\"".to_owned()),
            direct_url: Some(format!("http://{address}/manifest?secret=credential")),
            store_id: None,
        };

        let err = fetch_rrd_manifest_via_key(
            &re_uri::Origin::test(),
            manifest_key,
            &SegmentId::new("recording".to_owned()),
            None,
        )
        .await
        .unwrap_err();
        server_thread.join().unwrap();

        assert_eq!(err.kind, ApiErrorKind::FailedPrecondition);
        assert!(err.to_string().contains(SOURCE_CHANGED_MESSAGE));
        assert!(!err.to_string().contains("credential"));
    }
}
