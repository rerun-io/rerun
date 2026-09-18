//! The regular HTTP routes that we serve next to the gRPC interface.

use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, State},
};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use futures::StreamExt as _;
use hmac::{Hmac, Mac as _};
use http::StatusCode;
use re_protos::cloud::v1alpha1::ext::ObjectKey;
use sha2::Sha256;
use tokio::io::AsyncWriteExt as _;

pub async fn get_version() -> String {
    re_build_info::build_info!().to_string()
}

/// Grants have the form `{encoded_claims}.{signature}`.
type Grant = String;

#[derive(serde::Deserialize, serde::Serialize)]
struct UploadClaims {
    size_bytes: u64,
    object_key: String,
    expires_at: jiff::Timestamp,
}

type HmacSha256 = Hmac<Sha256>;

const MAX_UPLOAD_SIZE_BYTES: u64 = 100_000_000_000;
const GRANT_VALIDITY: jiff::SignedDuration = jiff::SignedDuration::from_mins(15);

#[derive(Clone)]
pub(crate) struct WriteAccessGrants {
    verifier: HmacSha256,
    storage_dir: PathBuf,
}

impl WriteAccessGrants {
    pub fn new(storage_dir: PathBuf) -> anyhow::Result<Self> {
        let mut key = hmac::digest::Key::<HmacSha256>::default();
        getrandom::fill(&mut key)
            .map_err(|err| anyhow::anyhow!("failed to generate upload HMAC key: {err}"))?;
        let verifier = HmacSha256::new(&key);

        Ok(Self {
            verifier,
            storage_dir,
        })
    }

    pub fn issue(
        &self,
        object_key: &ObjectKey,
        size_bytes: u64,
    ) -> tonic::Result<re_protos::cloud::v1alpha1::ext::GetWriteAccessGrantResponse> {
        use re_protos::cloud::v1alpha1::ext::{
            AccessGrant, GetWriteAccessGrantResponse, HttpRequest, Redemption,
        };

        if size_bytes > MAX_UPLOAD_SIZE_BYTES {
            return Err(tonic::Status::invalid_argument(format!(
                "object size exceeds the maximum of {MAX_UPLOAD_SIZE_BYTES} bytes"
            )));
        }

        let expires_at = jiff::Timestamp::now()
            .checked_add(GRANT_VALIDITY)
            .map_err(|err| tonic::Status::internal(format!("failed to set grant expiry: {err}")))?;
        let claims = bincode::serialize(&UploadClaims {
            size_bytes,
            object_key: object_key.to_string(),
            expires_at,
        })
        .map_err(|err| tonic::Status::internal(format!("failed to serialize grant: {err}")))?;

        let mut signer = self.verifier.clone();
        signer.update(&claims);
        let signature = signer.finalize().into_bytes();
        let grant = format!(
            "{}.{}",
            BASE64_URL_SAFE_NO_PAD.encode(claims),
            BASE64_URL_SAFE_NO_PAD.encode(signature)
        );

        let path_and_query = format!("/upload/{grant}").parse().map_err(|err| {
            tonic::Status::internal(format!("failed to build upload path: {err}"))
        })?;
        let storage_url = url::Url::from_file_path(self.storage_dir.join(object_key.to_string()))
            .map_err(|()| tonic::Status::internal("failed to build storage URL"))?;

        Ok(GetWriteAccessGrantResponse {
            storage_url,
            grant: AccessGrant {
                expires_at,
                redemption: Redemption::HttpRequest(HttpRequest::SameOrigin {
                    method: http::Method::PUT,
                    path_and_query,
                    headers: http::HeaderMap::new(),
                }),
            },
        })
    }
}

fn verify_signature(
    verifier: &HmacSha256,
    claims: &[u8],
    signature: &[u8],
) -> Result<(), hmac::digest::MacError> {
    let mut verifier = verifier.clone();
    verifier.update(claims);
    verifier.verify_slice(signature)
}

async fn write_upload(
    storage_dir: &std::path::Path,
    object_key: &ObjectKey,
    size_bytes: u64,
    body: Body,
) -> Result<(), (StatusCode, String)> {
    let destination_path = storage_dir.join(object_key.to_string());
    let parent = destination_path.parent().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Upload destination has no parent: {}",
                destination_path.display()
            ),
        )
    })?;
    tokio::fs::create_dir_all(parent).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to create upload directory: {err:#}\nPath: {}",
                parent.display()
            ),
        )
    })?;

    let temporary_file = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to create temporary upload file: {err:#}\nPath: {}",
                parent.display()
            ),
        )
    })?;
    let (file, temporary_path) = temporary_file.keep().map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to retain temporary upload file: {:#}\nPath: {}",
                err.error,
                parent.display()
            ),
        )
    })?;
    let file = tokio::fs::File::from_std(file);

    let result = async {
        write_upload_body(file, size_bytes, body)
            .await
            .map_err(|(status, message)| {
                (
                    status,
                    format!("{message}\nPath: {}", destination_path.display()),
                )
            })?;
        tokio::fs::hard_link(&temporary_path, &destination_path)
            .await
            .map_err(|err| {
                let status = match err.kind() {
                    std::io::ErrorKind::AlreadyExists => StatusCode::CONFLICT,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (
                    status,
                    format!(
                        "Failed to publish upload: {err:#}\nPath: {}",
                        destination_path.display()
                    ),
                )
            })
    }
    .await;

    if let Err(err) = tokio::fs::remove_file(&temporary_path).await {
        re_log::warn!(%object_key, path = %temporary_path.display(), "Failed to remove temporary upload: {err:#}");
    }

    result
}

async fn write_upload_body(
    mut file: tokio::fs::File,
    size_bytes: u64,
    body: Body,
) -> Result<(), (StatusCode, String)> {
    let mut body = body.into_data_stream();
    let mut bytes_written = 0_u64;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read upload body: {err:#}"),
            )
        })?;
        bytes_written = bytes_written
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| {
                (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Upload size exceeds u64::MAX bytes".to_owned(),
                )
            })?;
        if bytes_written > size_bytes {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Upload exceeds the granted size of {size_bytes} bytes"),
            ));
        }
        file.write_all(&chunk).await.map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write upload body: {err:#}"),
            )
        })?;
    }

    if bytes_written != size_bytes {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Upload size mismatch: expected {size_bytes} bytes, received {bytes_written}"),
        ));
    }
    file.flush().await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to flush upload file: {err:#}"),
        )
    })
}

fn upload_error_response((status, message): (StatusCode, String)) -> (StatusCode, String) {
    if status.is_server_error() {
        (status, "Internal server error".to_owned())
    } else {
        (status, message)
    }
}

pub fn write_upload_route(state: WriteAccessGrants) -> axum::routing::MethodRouter {
    axum::routing::put(put_upload)
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::disable())
}

#[tracing::instrument(skip_all)]
async fn put_upload(
    State(state): State<WriteAccessGrants>,
    Path(grant): Path<Grant>,
    body: Body,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some((encoded_claims, encoded_signature)) = grant.split_once('.') else {
        return Err((StatusCode::BAD_REQUEST, "Malformed upload grant".to_owned()));
    };

    let decoded_claims = BASE64_URL_SAFE_NO_PAD
        .decode(encoded_claims)
        .map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode upload claims: {err:#}"),
            )
        })?;

    let decoded_signature = BASE64_URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode upload signature: {err:#}"),
            )
        })?;

    verify_signature(&state.verifier, &decoded_claims, &decoded_signature).map_err(|err| {
        (
            StatusCode::FORBIDDEN,
            format!("Invalid upload signature: {err:#}"),
        )
    })?;

    let UploadClaims {
        size_bytes,
        object_key,
        expires_at,
    } = bincode::deserialize(&decoded_claims).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to deserialize upload claims: {err:#}"),
        )
    })?;
    let object_key = ObjectKey::try_new(object_key).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid upload object key: {err:#}"),
        )
    })?;
    if expires_at <= jiff::Timestamp::now() {
        return Err((StatusCode::FORBIDDEN, "Upload grant has expired".to_owned()));
    }

    write_upload(&state.storage_dir, &object_key, size_bytes, body)
        .await
        .map_err(|err| {
            re_log::warn!(%object_key, status = %err.0, "Upload failed: {}", err.1);
            upload_error_response(err)
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use re_protos::cloud::v1alpha1::ext::{HttpRequest, Redemption};

    use super::*;

    #[tokio::test]
    async fn writes_upload_to_object_key() {
        let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
        let object_key = ObjectKey::try_new("project/recording.rrd").expect("valid object key");

        write_upload(storage_dir.path(), &object_key, 7, Body::from("content"))
            .await
            .expect("upload should succeed");

        let (status, _) = write_upload(storage_dir.path(), &object_key, 3, Body::from("new"))
            .await
            .expect_err("overwriting should fail");
        assert_eq!(status, StatusCode::CONFLICT);

        let contents = tokio::fs::read(storage_dir.path().join("project/recording.rrd"))
            .await
            .expect("failed to read uploaded object");
        assert_eq!(contents, b"content");
    }

    #[tokio::test]
    async fn does_not_expose_upload_io_errors() {
        let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
        let parent = storage_dir.path().join("project");
        tokio::fs::write(&parent, b"not a directory")
            .await
            .expect("failed to create blocking file");
        let grants =
            WriteAccessGrants::new(storage_dir.path().to_owned()).expect("failed to create grants");
        let grant = grants
            .issue(
                &ObjectKey::try_new("project/recording.rrd").expect("valid object key"),
                7,
            )
            .expect("failed to issue grant");
        let Redemption::HttpRequest(HttpRequest::SameOrigin { path_and_query, .. }) =
            grant.grant.redemption
        else {
            panic!("expected same-origin HTTP grant");
        };
        let grant = path_and_query
            .path()
            .strip_prefix("/upload/")
            .expect("upload URL")
            .to_owned();

        let (status, message) = put_upload(State(grants), Path(grant), Body::from("content"))
            .await
            .expect_err("upload should fail");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(message, "Internal server error");
    }

    #[tokio::test]
    async fn reports_body_errors_and_removes_partial_upload() {
        let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
        let object_key = ObjectKey::try_new("recording.rrd").expect("valid object key");
        let body = Body::from_stream(futures::stream::iter([
            Ok(bytes::Bytes::from_static(b"partial")),
            Err(std::io::Error::other("interrupted upload")),
        ]));

        let (status, message) = write_upload(storage_dir.path(), &object_key, 10, body)
            .await
            .expect_err("upload should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(message.contains("Failed to read upload body"));
        assert!(message.contains("interrupted upload"));
        assert!(!storage_dir.path().join("recording.rrd").exists());
        assert!(
            tokio::fs::read_dir(storage_dir.path())
                .await
                .expect("failed to read storage directory")
                .next_entry()
                .await
                .expect("failed to read storage directory")
                .is_none()
        );
    }

    #[test]
    fn accepts_maximum_and_rejects_oversized_grants() {
        let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
        let grants =
            WriteAccessGrants::new(storage_dir.path().to_owned()).expect("failed to create grants");
        let object_key = ObjectKey::try_new("recording.rrd").expect("valid object key");

        grants
            .issue(&object_key, MAX_UPLOAD_SIZE_BYTES)
            .expect("maximum-sized grant should be accepted");

        let err = grants
            .issue(&object_key, MAX_UPLOAD_SIZE_BYTES + 1)
            .expect_err("oversized grant should be rejected");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn rejects_invalid_and_expired_grants() {
        let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
        let grants =
            WriteAccessGrants::new(storage_dir.path().to_owned()).expect("failed to create grants");
        let object_key = ObjectKey::try_new("recording.rrd").expect("valid object key");
        let grant = grants.issue(&object_key, 7).expect("failed to issue grant");
        let Redemption::HttpRequest(HttpRequest::SameOrigin { path_and_query, .. }) =
            grant.grant.redemption
        else {
            panic!("expected same-origin HTTP grant");
        };
        let grant = path_and_query
            .path()
            .strip_prefix("/upload/")
            .expect("upload URL")
            .to_owned();

        let (encoded_claims, _) = grant.split_once('.').expect("grant has signature");
        let invalid_grant = format!(
            "{encoded_claims}.{}",
            BASE64_URL_SAFE_NO_PAD.encode([0_u8; 32])
        );
        let (status, _) = put_upload(
            State(grants.clone()),
            Path(invalid_grant),
            Body::from("content"),
        )
        .await
        .expect_err("invalid grant should be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, _) = put_upload(
            State(grants.clone()),
            Path("malformed".to_owned()),
            Body::from("content"),
        )
        .await
        .expect_err("malformed grant should be rejected");
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let claims = UploadClaims {
            size_bytes: 7,
            object_key: object_key.to_string(),
            expires_at: jiff::Timestamp::now()
                .checked_sub(GRANT_VALIDITY)
                .expect("valid expired timestamp"),
        };
        let claims = bincode::serialize(&claims).expect("failed to serialize claims");
        let mut signer = grants.verifier.clone();
        signer.update(&claims);
        let expired_grant = format!(
            "{}.{}",
            BASE64_URL_SAFE_NO_PAD.encode(&claims),
            BASE64_URL_SAFE_NO_PAD.encode(signer.finalize().into_bytes())
        );
        let (status, _) = put_upload(State(grants), Path(expired_grant), Body::from("content"))
            .await
            .expect_err("expired grant should be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!storage_dir.path().join(object_key.to_string()).exists());
    }

    #[tokio::test]
    async fn rejects_upload_with_wrong_size() {
        for (claimed_size, expected_status) in [
            (8, StatusCode::BAD_REQUEST),
            (6, StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let storage_dir = tempfile::tempdir().expect("failed to create storage directory");
            let object_key = ObjectKey::try_new("recording.rrd").expect("valid object key");

            let (status, _) = write_upload(
                storage_dir.path(),
                &object_key,
                claimed_size,
                Body::from("content"),
            )
            .await
            .expect_err("upload should fail");

            assert_eq!(status, expected_status);
            assert!(!storage_dir.path().join("recording.rrd").exists());
        }
    }
}
